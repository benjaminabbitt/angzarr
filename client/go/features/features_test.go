package features

import (
	"os"
	"testing"

	"github.com/cucumber/godog"
	"github.com/cucumber/godog/colors"
)

var opts = godog.Options{
	Output:      colors.Colored(os.Stdout),
	Format:      "progress",
	Paths:       []string{"../../features"},
	Randomize:   0,
	Concurrency: 1,
}

func TestFeatures(t *testing.T) {
	suite := godog.TestSuite{
		ScenarioInitializer: InitializeScenario,
		Options:             &opts,
	}

	if suite.Run() != 0 {
		t.Fail()
	}
}

func InitializeScenario(ctx *godog.ScenarioContext) {
	// Command builder steps
	InitCommandBuilderSteps(ctx)

	// Query builder steps
	InitQueryBuilderSteps(ctx)

	// Router steps
	InitRouterSteps(ctx)

	// State building steps
	InitStateBuildingSteps(ctx)

	// Event decoding steps
	InitEventDecodingSteps(ctx)

	// Error handling steps
	InitErrorHandlingSteps(ctx)

	// Compensation steps
	InitCompensationSteps(ctx)

	// Aggregate client steps (router scenarios)
	InitializeAggregateScenario(ctx)

	// Connection steps
	InitConnectionSteps(ctx)

	// Query client steps
	InitQueryClientSteps(ctx)

	// Speculative client steps
	InitSpeculativeClientSteps(ctx)
}
