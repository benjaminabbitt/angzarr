rootProject.name = "angzarr-examples-java"

// Use the canonical client library from client/java via composite build
includeBuild("../../client/java") {
    dependencySubstitution {
        substitute(module("dev.angzarr:client")).using(project(":client"))
        substitute(module("dev.angzarr:proto")).using(project(":proto"))
    }
}

// Player domain
include("player-agg")
project(":player-agg").projectDir = file("player/agg")

// Table domain
include("table-agg")
project(":table-agg").projectDir = file("table/agg")
include("table-saga-hand")
project(":table-saga-hand").projectDir = file("table/saga-hand")
include("table-saga-player")
project(":table-saga-player").projectDir = file("table/saga-player")

// Hand domain
include("hand-agg")
project(":hand-agg").projectDir = file("hand/agg")
include("hand-saga-table")
project(":hand-saga-table").projectDir = file("hand/saga-table")
include("hand-saga-player")
project(":hand-saga-player").projectDir = file("hand/saga-player")

// Process Manager
include("hand-flow")

// Projector
include("prj-output")

// Tests
include("tests")

// Configure proto path resolution
pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        mavenCentral()
    }
}
