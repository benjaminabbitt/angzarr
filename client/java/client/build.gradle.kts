plugins {
    `java-library`
    id("org.springframework.boot") version "3.2.0" apply false
    id("io.spring.dependency-management") version "1.1.4"
    `maven-publish`
}

dependencyManagement {
    imports {
        mavenBom("org.springframework.boot:spring-boot-dependencies:3.2.0")
    }
}

dependencies {
    api(project(":proto"))

    // Spring Boot (optional - for DI when running as server)
    compileOnly("org.springframework.boot:spring-boot-starter")

    // gRPC
    implementation("io.grpc:grpc-netty-shaded:1.60.0")
    implementation("io.grpc:grpc-protobuf:1.60.0")
    implementation("io.grpc:grpc-stub:1.60.0")
    implementation("net.devh:grpc-spring-boot-starter:2.15.0.RELEASE")

    // Protobuf
    implementation("com.google.protobuf:protobuf-java:3.25.1")
    implementation("com.google.protobuf:protobuf-java-util:3.25.1")

    // Logging
    implementation("org.slf4j:slf4j-api:2.0.9")

    // Testing
    testImplementation("org.junit.jupiter:junit-jupiter:5.10.1")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
    testImplementation("org.assertj:assertj-core:3.24.2")

    // Cucumber
    testImplementation("io.cucumber:cucumber-java:7.15.0")
    testImplementation("io.cucumber:cucumber-junit-platform-engine:7.15.0")
    testImplementation("org.junit.platform:junit-platform-suite:1.10.1")
}

tasks.test {
    useJUnitPlatform()
    systemProperty("cucumber.junit-platform.naming-strategy", "long")
}

// Copy shared feature files to test resources
tasks.register<Copy>("copyClientFeatures") {
    from("${rootDir}/../../tests/client/features")
    into("src/test/resources/features")
}

tasks.named("processTestResources") {
    dependsOn("copyClientFeatures")
}

publishing {
    publications {
        create<MavenPublication>("maven") {
            groupId = "dev.angzarr"
            artifactId = "angzarr-client"
            from(components["java"])

            pom {
                name.set("Angzarr Client")
                description.set("Java client library for Angzarr gRPC services")
                url.set("https://github.com/angzarr-io/angzarr")

                licenses {
                    license {
                        name.set("AGPL-3.0-only")
                        url.set("https://www.gnu.org/licenses/agpl-3.0.html")
                    }
                }
            }
        }
    }
}
