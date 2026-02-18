import com.google.protobuf.gradle.*

plugins {
    `java-library`
    id("com.google.protobuf") version "0.9.4"
}

dependencies {
    api("io.grpc:grpc-protobuf:1.60.0")
    api("io.grpc:grpc-stub:1.60.0")
    api("com.google.protobuf:protobuf-java:3.25.1")
    implementation("javax.annotation:javax.annotation-api:1.3.2")
}

protobuf {
    protoc {
        path = "/usr/bin/protoc"
    }
    plugins {
        id("grpc") {
            artifact = "io.grpc:protoc-gen-grpc-java:1.60.0"
        }
    }
    generateProtoTasks {
        all().forEach { task ->
            task.plugins {
                id("grpc")
            }
        }
    }
}

sourceSets {
    main {
        proto {
            // Use proto root so imports like "angzarr/types.proto" resolve
            srcDir("../../../proto")
            // Exclude health protos - not needed for Java client
            exclude("health/**")
        }
    }
}
