package tests

import (
	"context"
	"errors"
	"fmt"
	"strings"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	"github.com/cucumber/godog"
)

// CommonContext holds shared state across scenarios
type CommonContext struct {
	LastError error
}

var sharedContext = &CommonContext{}

// SetLastError stores the last error for assertion in common steps
func SetLastError(err error) {
	sharedContext.LastError = err
}

// InitCommonSteps registers common step definitions shared across aggregates
func InitCommonSteps(ctx *godog.ScenarioContext) {
	// Reset before each scenario
	ctx.Before(func(c context.Context, sc *godog.Scenario) (context.Context, error) {
		sharedContext.LastError = nil
		return c, nil
	})

	// Shared step for checking command failures (used across all aggregates)
	ctx.Step(`^the command fails with status "([^"]*)"$`, commandFailsWithStatus)
	ctx.Step(`^the error message contains "([^"]*)"$`, errorMessageContains)
}

func commandFailsWithStatus(status string) error {
	if sharedContext.LastError == nil {
		return fmt.Errorf("expected command to fail with status %s, but it succeeded", status)
	}
	// Check if it's a CommandRejectedError (value type, not pointer)
	var cmdErr angzarr.CommandRejectedError
	if !errors.As(sharedContext.LastError, &cmdErr) {
		return fmt.Errorf("expected CommandRejectedError, got %T: %v", sharedContext.LastError, sharedContext.LastError)
	}
	return nil
}

func errorMessageContains(text string) error {
	if sharedContext.LastError == nil {
		return fmt.Errorf("expected an error but got success")
	}
	errMsg := strings.ToLower(sharedContext.LastError.Error())
	if !strings.Contains(errMsg, strings.ToLower(text)) {
		return fmt.Errorf("expected error to contain '%s', got '%s'", text, sharedContext.LastError.Error())
	}
	return nil
}
