/// Cucumber-cpp test runner for C++ poker examples.
///
/// This is the main entry point for running BDD acceptance tests.
/// The step definitions are in separate *_steps.cpp files and are
/// automatically registered via cucumber-cpp's autodetect mechanism.

// GTest must be included before cucumber-cpp autodetect for framework detection
#include <gtest/gtest.h>
#include <cucumber-cpp/autodetect.hpp>

// The autodetect header handles everything - it provides a main()
// that sets up the cucumber test infrastructure and runs the features.

// Feature files are located in ../features/ (symlinks to ../../features/unit/)
// and are passed to cucumber via command line or environment variable.
