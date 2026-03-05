package tests

import (
	"os"
	"testing"

	"github.com/cucumber/godog"
	"github.com/cucumber/godog/colors"
)

var opts = godog.Options{
	Output:      colors.Colored(os.Stdout),
	Format:      "progress",
	Paths:       []string{"../../features/unit"},
	Randomize:   0,
	Concurrency: 1,
	Strict:      false, // Allow pending scenarios without failing
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
	// Player aggregate steps
	InitPlayerSteps(ctx)

	// Table aggregate steps
	InitTableSteps(ctx)

	// Hand aggregate steps
	InitHandSteps(ctx)

	// Saga steps
	RegisterSagaSteps(ctx)

	// Process manager steps
	RegisterPMSteps(ctx)

	// Projector steps
	RegisterProjectorSteps(ctx)

	// Common steps (shared assertions)
	InitCommonSteps(ctx)
}
