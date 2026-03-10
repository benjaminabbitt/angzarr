plugins {
    java
}

dependencies {
    // Composite build dependencies - use implementation to ensure dependency
    // substitution works correctly (testImplementation alone can fail in CI
    // when the module has no main sources)
    implementation("dev.angzarr:client")
    implementation("dev.angzarr:proto")

    // Project dependencies
    testImplementation(project(":player-agg"))
    testImplementation(project(":table-agg"))
    testImplementation(project(":hand-agg"))

    // Cucumber
    testImplementation("io.cucumber:cucumber-java:7.15.0")
    testImplementation("io.cucumber:cucumber-junit-platform-engine:7.15.0")
    testImplementation("io.cucumber:cucumber-spring:7.15.0")

    // Spring Boot Test (for DI in tests)
    testImplementation("org.springframework.boot:spring-boot-starter-test:3.2.0")

    // JUnit Platform
    testImplementation("org.junit.platform:junit-platform-suite:1.10.1")
    testImplementation("org.junit.jupiter:junit-jupiter:5.10.1")

    // Assertions
    testImplementation("org.assertj:assertj-core:3.24.2")

    // gRPC for Status
    testImplementation("io.grpc:grpc-api:1.60.0")
}

tasks.test {
    useJUnitPlatform()
    systemProperty("cucumber.junit-platform.naming-strategy", "long")
}

// Feature files are symlinked from examples/features/unit/
// Step definitions match the shared feature format
