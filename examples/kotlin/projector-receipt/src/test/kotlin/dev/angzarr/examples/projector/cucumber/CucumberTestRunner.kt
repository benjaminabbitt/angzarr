package dev.angzarr.examples.projector.cucumber

import org.junit.platform.suite.api.ConfigurationParameter
import org.junit.platform.suite.api.IncludeEngines
import org.junit.platform.suite.api.SelectClasspathResource
import org.junit.platform.suite.api.Suite

@Suite
@IncludeEngines("cucumber")
@SelectClasspathResource("features")
@ConfigurationParameter(key = "cucumber.glue", value = "dev.angzarr.examples.projector.cucumber")
@ConfigurationParameter(key = "cucumber.plugin", value = "pretty")
class CucumberTestRunner
