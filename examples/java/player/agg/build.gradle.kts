plugins {
    java
    application
    id("org.springframework.boot") version "3.2.0"
    id("io.spring.dependency-management") version "1.1.4"
}

dependencies {
    implementation("dev.angzarr:client")
    implementation("dev.angzarr:proto")

    // Spring Boot
    implementation("org.springframework.boot:spring-boot-starter")
    implementation("net.devh:grpc-spring-boot-starter:2.15.0.RELEASE")

    // Testing
    testImplementation("org.springframework.boot:spring-boot-starter-test")
    testImplementation("org.junit.jupiter:junit-jupiter:5.10.1")
    testImplementation("org.assertj:assertj-core:3.24.2")
}

application {
    mainClass.set("dev.angzarr.examples.player.Main")
}

springBoot {
    mainClass.set("dev.angzarr.examples.player.Main")
}
