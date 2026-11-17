plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

android {
    namespace = "cd.cc.nongma.vrc"
    compileSdk {
        version = release(37)
    }

    defaultConfig {
        applicationId = "cd.cc.nongma.vrc"
        minSdk = 31
        targetSdk = 37
        versionCode = 1
        versionName = "1.0"

        // 目标设备（骁龙 8 Gen 级）为 arm64-v8a；限制 ABI 避免 APK 内含 4 套 so
        ndk {
            abiFilters += listOf("arm64-v8a")
        }

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    // SenseVoice int8 模型已是量化格式，再压缩几乎无收益，跳过节省打包时间
    androidResources {
        noCompress += "onnx"
    }

    buildTypes {
        release {
            // 本地自用：release 变体用 debug 签名（项目暂无独立 release 密钥）
            signingConfig = signingConfigs.getByName("debug")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    buildFeatures {
        compose = true
    }
}

// AGP 9 内置的 Kotlin 编译器（2.2.x，能读 metadata ≤ 2.3.0）。
// composeBom 2026.02.01 会传递引入 kotlin-stdlib 2.4.x（metadata 2.4.0，内置编译器读不了），
// 强制锁定到 2.3.21（metadata 2.3.0，同时满足 backdrop 2.0.0 的依赖）。
configurations.all {
    resolutionStrategy {
        force("org.jetbrains.kotlin:kotlin-stdlib:2.3.21")
        force("org.jetbrains.kotlin:kotlin-stdlib-jdk7:2.3.21")
        force("org.jetbrains.kotlin:kotlin-stdlib-jdk8:2.3.21")
        force("org.jetbrains.kotlin:kotlin-stdlib-common:2.3.21")
    }
}

dependencies {
    // 端侧离线识别（SenseVoice）：本地 AAR，避免发布到公共仓库
    implementation(files("libs/sherpa-onnx-2.0.1.aar"))
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.activity.compose)
    implementation("io.github.kyant0:backdrop:2.0.0")
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("com.journeyapps:zxing-android-embedded:4.3.0")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-core:1.7.3")
    testImplementation(libs.junit)
    androidTestImplementation(platform(libs.androidx.compose.bom))
    androidTestImplementation(libs.androidx.compose.ui.test.junit4)
    androidTestImplementation(libs.androidx.espresso.core)
    androidTestImplementation(libs.androidx.junit)
    debugImplementation(libs.androidx.compose.ui.test.manifest)
    debugImplementation(libs.androidx.compose.ui.tooling)
}