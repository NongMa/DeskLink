package cd.cc.nongma.vrc.audio

import android.content.Context
import android.util.Log
import com.k2fsa.sherpa.onnx.FeatureConfig
import com.k2fsa.sherpa.onnx.OfflineModelConfig
import com.k2fsa.sherpa.onnx.OfflineRecognizer
import com.k2fsa.sherpa.onnx.OfflineRecognizerConfig
import com.k2fsa.sherpa.onnx.OfflineSenseVoiceModelConfig
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

/**
 * 手机端离线识别（sherpa-onnx + SenseVoice int8）。
 *
 * 模型随 APK 打包在 assets/sense-voice/（model.int8.onnx + tokens.txt），
 * 由 sherpa-onnx 的 AssetManager 直接加载，无需复制到 filesDir。
 * 首次 decode 前约 1~3 秒的模型加载在 initAsync 预热里完成。
 *
 * 任何一步失败都会返回 null，调用方给出本地「没听清」提示。
 */
object AsrRecognizer {
    private const val TAG = "AsrRecognizer"
    private const val ASSET_MODEL = "sense-voice/model.int8.onnx"
    private const val ASSET_TOKENS = "sense-voice/tokens.txt"

    private val lock = Any()
    @Volatile private var recognizer: OfflineRecognizer? = null
    @Volatile private var initFailed = false

    /** 后台预热模型，避免首次按住麦克风时再加载 226MB 权重 */
    fun initAsync(context: Context) {
        CoroutineScope(Dispatchers.Default).launch { ensureRecognizer(context) }
    }

    fun isReady(): Boolean = recognizer != null

    private fun ensureRecognizer(context: Context): OfflineRecognizer? {
        recognizer?.let { return it }
        if (initFailed) return null
        synchronized(lock) {
            recognizer?.let { return it }
            if (initFailed) return null
            return try {
                val config = OfflineRecognizerConfig(
                    featConfig = FeatureConfig(sampleRate = 16000, featureDim = 80),
                    modelConfig = OfflineModelConfig(
                        senseVoice = OfflineSenseVoiceModelConfig(
                            model = ASSET_MODEL,
                            language = "zh",
                            useInverseTextNormalization = true,
                        ),
                        tokens = ASSET_TOKENS,
                        numThreads = 4,
                        provider = "cpu",
                        debug = false,
                    ),
                )
                OfflineRecognizer(context.applicationContext.assets, config)
                    .also { recognizer = it }
                    .also { Log.i(TAG, "SenseVoice recognizer ready") }
            } catch (t: Throwable) {
                Log.e(TAG, "recognizer init failed", t)
                initFailed = true
                null
            }
        }
    }

    /**
     * 识别 PCM16（小端、16kHz 单声道）音频。
     * @return 识别文本（已 trim）；识别失败/无结果返回 null（调用方给出本地「没听清」提示）
     */
    fun recognizeAsync(context: Context, pcm16: ByteArray, onResult: (String?) -> Unit) {
        CoroutineScope(Dispatchers.Default).launch {
            val text = runCatching {
                val rec = ensureRecognizer(context) ?: return@runCatching null
                val count = pcm16.size / 2
                if (count == 0) return@runCatching null
                val samples = FloatArray(count)
                var out = 0
                var inIdx = 0
                while (inIdx + 1 < pcm16.size) {
                    val s = ((pcm16[inIdx + 1].toInt() shl 8) or (pcm16[inIdx].toInt() and 0xff)).toShort()
                    samples[out++] = s / 32768.0f
                    inIdx += 2
                }
                val stream = rec.createStream()
                try {
                    stream.acceptWaveform(samples, 16000)
                    rec.decode(stream)
                    rec.getResult(stream).text.trim()
                } finally {
                    stream.release()
                }
            }.onFailure {
                Log.e(TAG, "recognize failed", it)
            }.getOrNull()

            val usable = text?.takeIf { it.isNotBlank() && it.any { c -> c.isLetterOrDigit() } }
            if (!usable.isNullOrBlank()) Log.i(TAG, "recognized (phone asr): $usable")
            else if (!text.isNullOrBlank()) Log.i(TAG, "ignoring non-speech result: $text")
            onResult(usable)
        }
    }
}
