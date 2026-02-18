plugins {
    java
    application
    id("org.springframework.boot") version "3.2.0"
    id("io.spring.dependency-management") version "1.1.4"
}

dependencies {
    implementation("dev.angzarr:client")
    implementation("dev.angzarr:proto")
    implementation("org.springframework.boot:spring-boot-starter")
    implementation("net.devh:grpc-spring-boot-starter:2.15.0.RELEASE")
}

application {
    mainClass.set("dev.angzarr.examples.handflowoo.Main")
}
