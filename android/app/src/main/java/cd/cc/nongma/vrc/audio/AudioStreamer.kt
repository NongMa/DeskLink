package cd.cc.nongma.vrc.audio

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import androidx.core.content.ContextCompat
import cd.cc.nongma.vrc.net.ConnectionStatus
import cd.cc.nongma.vrc.net.VrcWebSocket
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.util.Base64
import java.util.concurrent.atomic.AtomicBoolean

object AudioStreamer {
    private val _recording = MutableStateFlow(false)
    val recording: StateFlow<Boolean> = _recording.asStateFlow()

    /** 麦克风电平 0..1（Siri 式声波动效用），录音时每 20ms 更新并做指数平滑 */
    private val _level = MutableStateFlow(0f)
    val level: StateFlow<Float> = _level.asStateFlow()

    private var job: Job? = null
    private var record: AudioRecord? = null
    private var appContext: Context? = null

    /** 本次录音的完整 PCM16 数据（松手后交给手机端识别）；与推流共用一份读取结果 */
    private val pcmBuffer = mutableListOf<ByteArray>()

    /** 部分识别（边说边识别）进行中标志，防止解码任务堆积 */
    private val partialDecoding = AtomicBoolean(false)
    /** 上次触发部分识别的时间戳（ms） */
    @Volatile private var lastPartialMs = 0L

    fun start(context: Context) {
        if (ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.RECORD_AUDIO
            ) != PackageManager.PERMISSION_GRANTED
        ) {
            _recording.value = false
            return
        }
        if (_recording.value) return
        appContext = context.applicationContext
        synchronized(pcmBuffer) { pcmBuffer.clear() }
        _recording.value = true
        lastPartialMs = System.currentTimeMillis()
        // 只有未连接/未在连接时才发起连接，避免按一次麦克风就把现有连接断开重连
        if (VrcWebSocket.status.value != ConnectionStatus.Connected &&
            VrcWebSocket.status.value != ConnectionStatus.Connecting
        ) {
            VrcWebSocket.connect()
        }

        job = CoroutineScope(Dispatchers.IO).launch {
            val sampleRate = 16000
            val channel = AudioFormat.CHANNEL_IN_MONO
            val format = AudioFormat.ENCODING_PCM_16BIT
            val minBuf = AudioRecord.getMinBufferSize(sampleRate, channel, format)
            record = AudioRecord(MediaRecorder.AudioSource.MIC, sampleRate, channel, format, minBuf)
            record?.startRecording()
            VrcWebSocket.notifyListening()
            val buf = ByteArray(640) // 20ms

            while (isActive) {
                val read = record?.read(buf, 0, buf.size) ?: 0
                if (read > 0) {
                    val bytes = buf.copyOf(read)
                    synchronized(pcmBuffer) { pcmBuffer.add(bytes) }
                    val b64 = Base64.getEncoder().encodeToString(bytes)
                    VrcWebSocket.sendAudioChunk(b64)
                    // RMS 电平（i16 小端），归一化 + 指数平滑喂给声波动效
                    val count = read / 2
                    if (count > 0) {
                        var sum = 0.0
                        var i = 0
                        while (i + 1 < read) {
                            val s = ((buf[i + 1].toInt() shl 8) or (buf[i].toInt() and 0xff)).toShort().toInt()
                            sum += s.toDouble() * s
                            i += 2
                        }
                        val rms = kotlin.math.sqrt(sum / count)
                        val target = (rms / 6000.0).coerceIn(0.0, 1.0).toFloat()
                        _level.value = _level.value * 0.6f + target * 0.4f
                    }
                }
                // 边说话边识别：每 700ms 对最近 ≤8s 的音频快照重解码一次（SenseVoice 离线模型，非流式）
                val nowMs = System.currentTimeMillis()
                if (nowMs - lastPartialMs >= 700) {
                    lastPartialMs = nowMs
                    maybePostPartial()
                }
            }
        }
    }

    fun stop() {
        val wasRecording = _recording.value
        _recording.value = false
        _level.value = 0f
        job?.cancel()
        job = null
        record?.let {
            runCatching { it.stop() }
            it.release()
        }
        record = null
        // 快照本次 PCM（推流给 PC 的同时，留一份给手机端识别）
        val pcmBytes = synchronized(pcmBuffer) {
            val total = pcmBuffer.sumOf { it.size }
            val out = ByteArray(total)
            var off = 0
            for (chunk in pcmBuffer) {
                System.arraycopy(chunk, 0, out, off, chunk.size)
                off += chunk.size
            }
            pcmBuffer.clear()
            out
        }
        if (wasRecording) {
            // 手机端识别：需要 WS 之外的本地模型，与连接状态无关
            val ctx = appContext
            if (ctx != null && pcmBytes.size >= 3200 /* ≥100ms */) {
                VrcWebSocket.notifyProcessing()
                AsrRecognizer.recognizeAsync(ctx, pcmBytes) { text ->
                    if (!text.isNullOrBlank()) {
                        // 识别成功：只发送文本，不让 PC 再跑 Whisper（server 收到 voice_command 会清空推流缓冲）
                        VrcWebSocket.sendVoiceCommand(text)
                    } else {
                        // 端侧识别失败/未识别出文字：回退 PC whisper
                        VrcWebSocket.sendEndOfSpeech()
                    }
                }
            } else {
                // 太短或没采到上下文：走 PC whisper 路径（它有自己的静音检测）
                VrcWebSocket.sendEndOfSpeech()
            }
        }
    }

    /** 边说话边识别：对最近 ≤8s 的 PCM 快照异步重解码，刷新 VrcWebSocket.voicePartial */
    private fun maybePostPartial() {
        if (!_recording.value) return
        val ctx = appContext ?: return
        // 已有部分识别在跑则跳过本次，避免 CPU 堆积
        if (!partialDecoding.compareAndSet(false, true)) return
        val snapshot = snapshotRecent(8 * 16000 * 2)
        if (snapshot == null) {
            partialDecoding.set(false)
            return
        }
        AsrRecognizer.recognizeAsync(ctx, snapshot) { text ->
            try {
                if (_recording.value && !text.isNullOrBlank()) {
                    VrcWebSocket.setVoicePartial(text)
                }
            } finally {
                partialDecoding.set(false)
            }
        }
    }

    /** 取 pcmBuffer 尾部不超过 maxBytes 的 PCM（从末尾倒推拷贝） */
    private fun snapshotRecent(maxBytes: Int): ByteArray? {
        val chunks = synchronized(pcmBuffer) { ArrayList(pcmBuffer) }
        if (chunks.isEmpty()) return null
        var total = 0
        val picked = ArrayList<ByteArray>()
        for (i in chunks.indices.reversed()) {
            val chunk = chunks[i]
            if (total + chunk.size > maxBytes) break
            picked.add(chunk)
            total += chunk.size
        }
        if (total == 0) return null
        val out = ByteArray(total)
        var off = total
        for (chunk in picked) {
            off -= chunk.size
            System.arraycopy(chunk, 0, out, off, chunk.size)
        }
        return out
    }

    /** 取消本次聆听（不发送 end_of_speech，直接复位到 Idle） */
    fun cancel() {
        _recording.value = false
        _level.value = 0f
        job?.cancel()
        job = null
        record?.let {
            runCatching { it.stop() }
            it.release()
        }
        record = null
        synchronized(pcmBuffer) { pcmBuffer.clear() }
        VrcWebSocket.resetVoiceState()
    }
}
