package dev.angzarr.examples.steps;

import io.cucumber.spring.CucumberContextConfiguration;
import org.springframework.boot.test.context.SpringBootTest;

/**
 * Cucumber Spring configuration for test execution.
 */
@CucumberContextConfiguration
@SpringBootTest(classes = CucumberSpringConfiguration.class)
public class CucumberSpringConfiguration {
}
