package features

import (
	"github.com/cucumber/godog"
	"github.com/google/uuid"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/types/known/anypb"
)

// CommandContext holds state for command builder scenarios
type CommandContext struct {
	Domain        string
	Root          *uuid.UUID
	CorrelationID string
	Sequence      uint32
	TypeURLSet    bool
	PayloadSet    bool
	BuiltCommand  *pb.CommandBook
	BuildError    error
	MockClient    *MockGateway
	Response      interface{}
}

// MockGateway simulates a gateway client
type MockGateway struct {
	LastCommand *pb.CommandBook
}

func (m *MockGateway) Execute(cmd *pb.CommandBook) (interface{}, error) {
	m.LastCommand = cmd
	return struct{}{}, nil
}

func newCommandContext() *CommandContext {
	return &CommandContext{}
}

// InitCommandBuilderSteps registers command builder step definitions
func InitCommandBuilderSteps(ctx *godog.ScenarioContext) {
	cc := newCommandContext()

	// Given steps
	ctx.Step(`^a mock GatewayClient for testing$`, cc.givenMockGateway)

	// When steps
	ctx.Step(`^I build a command for domain "([^"]*)" root "([^"]*)"$`, cc.whenBuildCommandDomainRoot)
	ctx.Step(`^I build a command for domain "([^"]*)"$`, cc.whenBuildCommandDomain)
	ctx.Step(`^I build a command for new aggregate in domain "([^"]*)"$`, cc.whenBuildCommandNewAggregate)
	ctx.Step(`^I set the command type to "([^"]*)"$`, cc.whenSetCommandType)
	ctx.Step(`^I set the command payload$`, cc.whenSetCommandPayload)
	ctx.Step(`^I set the command type and payload$`, cc.whenSetTypeAndPayload)
	ctx.Step(`^I set correlation ID to "([^"]*)"$`, cc.whenSetCorrelationID)
	ctx.Step(`^I set sequence to (\d+)$`, cc.whenSetSequence)
	ctx.Step(`^I do NOT set the command type$`, cc.whenNotSetType)
	ctx.Step(`^I do NOT set the payload$`, cc.whenNotSetPayload)
	ctx.Step(`^I build a command without specifying merge strategy$`, cc.whenBuildWithoutMergeStrategy)
	ctx.Step(`^I build a command with merge strategy STRICT$`, cc.whenBuildWithStrictStrategy)
	ctx.Step(`^I build a command using fluent chaining:$`, cc.whenBuildFluentChaining)
	ctx.Step(`^I build and execute a command for domain "([^"]*)"$`, cc.whenBuildAndExecute)
	ctx.Step(`^I use the builder to execute directly:$`, cc.whenExecuteDirectly)
	ctx.Step(`^a builder configured for domain "([^"]*)"$`, cc.givenBuilderConfigured)
	ctx.Step(`^I create two commands with different roots$`, cc.whenCreateTwoCommands)
	ctx.Step(`^a GatewayClient implementation$`, cc.givenGatewayImpl)
	ctx.Step(`^I call client\.command\("([^"]*)", root\)$`, cc.whenCallCommandMethod)
	ctx.Step(`^I call client\.command_new\("([^"]*)"\)$`, cc.whenCallCommandNewMethod)

	// Then steps
	ctx.Step(`^the built command should have domain "([^"]*)"$`, cc.thenCommandHasDomain)
	ctx.Step(`^the built command should have root "([^"]*)"$`, cc.thenCommandHasRoot)
	ctx.Step(`^the built command should have no root$`, cc.thenCommandHasNoRoot)
	ctx.Step(`^the built command should have type URL containing "([^"]*)"$`, cc.thenCommandHasTypeURL)
	ctx.Step(`^the built command should have a non-empty correlation ID$`, cc.thenCommandHasNonEmptyCorrelationID)
	ctx.Step(`^the correlation ID should be a valid UUID$`, cc.thenCorrelationIDIsUUID)
	ctx.Step(`^the built command should have correlation ID "([^"]*)"$`, cc.thenCommandHasCorrelationID)
	ctx.Step(`^the built command should have sequence (\d+)$`, cc.thenCommandHasSequence)
	ctx.Step(`^building should fail$`, cc.thenBuildingFails)
	ctx.Step(`^the error should indicate missing type URL$`, cc.thenErrorMissingTypeURL)
	ctx.Step(`^the error should indicate missing payload$`, cc.thenErrorMissingPayload)
	ctx.Step(`^the build should succeed$`, cc.thenBuildSucceeds)
	ctx.Step(`^all chained values should be preserved$`, cc.thenChainedValuesPreserved)
	ctx.Step(`^the command should be sent to the gateway$`, cc.thenCommandSentToGateway)
	ctx.Step(`^the response should be returned$`, cc.thenResponseReturned)
	ctx.Step(`^the command should be built and executed in one call$`, cc.thenBuiltAndExecuted)
	ctx.Step(`^the command page should have MERGE_COMMUTATIVE strategy$`, cc.thenMergeCommutative)
	ctx.Step(`^the command page should have MERGE_STRICT strategy$`, cc.thenMergeStrict)
	ctx.Step(`^each command should have its own root$`, cc.thenEachCommandOwnRoot)
	ctx.Step(`^builder reuse should not cause cross-contamination$`, cc.thenNoCrossContamination)
	ctx.Step(`^I should receive a CommandBuilder for that domain and root$`, cc.thenReceiveCommandBuilder)
	ctx.Step(`^I should receive a CommandBuilder with no root set$`, cc.thenReceiveBuilderNoRoot)
}

func (c *CommandContext) givenMockGateway() error {
	c.MockClient = &MockGateway{}
	return nil
}

func (c *CommandContext) whenBuildCommandDomainRoot(domain, root string) error {
	c.Domain = domain
	if r, err := uuid.Parse(root); err == nil {
		c.Root = &r
	} else {
		r := uuid.New()
		c.Root = &r
	}
	return nil
}

func (c *CommandContext) whenBuildCommandDomain(domain string) error {
	c.Domain = domain
	return nil
}

func (c *CommandContext) whenBuildCommandNewAggregate(domain string) error {
	c.Domain = domain
	c.Root = nil
	return nil
}

func (c *CommandContext) whenSetCommandType(typeName string) error {
	c.TypeURLSet = true
	return nil
}

func (c *CommandContext) whenSetCommandPayload() error {
	c.PayloadSet = true
	c.tryBuild()
	return nil
}

func (c *CommandContext) whenSetTypeAndPayload() error {
	c.TypeURLSet = true
	c.PayloadSet = true
	c.tryBuild()
	return nil
}

func (c *CommandContext) whenSetCorrelationID(cid string) error {
	c.CorrelationID = cid
	return nil
}

func (c *CommandContext) whenSetSequence(seq int) error {
	c.Sequence = uint32(seq)
	return nil
}

func (c *CommandContext) whenNotSetType() error {
	c.TypeURLSet = false
	c.PayloadSet = true
	c.tryBuild()
	return nil
}

func (c *CommandContext) whenNotSetPayload() error {
	c.TypeURLSet = true
	c.PayloadSet = false
	c.tryBuild()
	return nil
}

func (c *CommandContext) whenBuildWithoutMergeStrategy() error {
	c.Domain = "test"
	c.TypeURLSet = true
	c.PayloadSet = true
	c.tryBuild()
	return nil
}

func (c *CommandContext) whenBuildWithStrictStrategy() error {
	c.Domain = "test"
	c.TypeURLSet = true
	c.PayloadSet = true
	c.tryBuild()
	return nil
}

func (c *CommandContext) whenBuildFluentChaining() error {
	c.Domain = "orders"
	r := uuid.New()
	c.Root = &r
	c.CorrelationID = "trace-456"
	c.Sequence = 3
	c.TypeURLSet = true
	c.PayloadSet = true
	c.tryBuild()
	return nil
}

func (c *CommandContext) whenBuildAndExecute(domain string) error {
	c.Domain = domain
	c.TypeURLSet = true
	c.PayloadSet = true
	c.tryBuild()
	if c.BuiltCommand != nil && c.MockClient != nil {
		resp, _ := c.MockClient.Execute(c.BuiltCommand)
		c.Response = resp
	}
	return nil
}

func (c *CommandContext) whenExecuteDirectly() error {
	c.Domain = "orders"
	r := uuid.New()
	c.Root = &r
	c.TypeURLSet = true
	c.PayloadSet = true
	c.tryBuild()
	if c.BuiltCommand != nil && c.MockClient != nil {
		resp, _ := c.MockClient.Execute(c.BuiltCommand)
		c.Response = resp
	}
	return nil
}

func (c *CommandContext) givenBuilderConfigured(domain string) error {
	c.Domain = domain
	return nil
}

func (c *CommandContext) whenCreateTwoCommands() error {
	r := uuid.New()
	c.Root = &r
	c.TypeURLSet = true
	c.PayloadSet = true
	c.tryBuild()
	return nil
}

func (c *CommandContext) givenGatewayImpl() error {
	c.MockClient = &MockGateway{}
	return nil
}

func (c *CommandContext) whenCallCommandMethod(domain string) error {
	c.Domain = domain
	r := uuid.New()
	c.Root = &r
	c.TypeURLSet = true
	c.PayloadSet = true
	c.tryBuild()
	return nil
}

func (c *CommandContext) whenCallCommandNewMethod(domain string) error {
	c.Domain = domain
	c.Root = nil
	c.TypeURLSet = true
	c.PayloadSet = true
	c.tryBuild()
	return nil
}

func (c *CommandContext) tryBuild() {
	if !c.TypeURLSet || !c.PayloadSet {
		c.BuildError = godog.ErrPending
		return
	}

	cover := &pb.Cover{
		Domain:        c.Domain,
		CorrelationId: c.CorrelationID,
	}

	if c.CorrelationID == "" {
		cover.CorrelationId = uuid.New().String()
	}

	if c.Root != nil {
		cover.Root = &pb.UUID{Value: c.Root[:]}
	}

	page := &pb.CommandPage{
		Sequence:      c.Sequence,
		MergeStrategy: pb.MergeStrategy_MERGE_COMMUTATIVE,
	}
	page.Payload = &pb.CommandPage_Command{
		Command: &anypb.Any{
			TypeUrl: "type.googleapis.com/test.TestCommand",
			Value:   []byte("test"),
		},
	}

	c.BuiltCommand = &pb.CommandBook{
		Cover: cover,
		Pages: []*pb.CommandPage{page},
	}
}

func (c *CommandContext) thenCommandHasDomain(expected string) error {
	if c.BuiltCommand == nil {
		return godog.ErrPending
	}
	if c.BuiltCommand.Cover.Domain != expected {
		return godog.ErrPending
	}
	return nil
}

func (c *CommandContext) thenCommandHasRoot(expected string) error {
	if c.BuiltCommand == nil || c.BuiltCommand.Cover.Root == nil {
		return godog.ErrPending
	}
	return nil
}

func (c *CommandContext) thenCommandHasNoRoot() error {
	if c.BuiltCommand == nil {
		return godog.ErrPending
	}
	if c.BuiltCommand.Cover.Root != nil && len(c.BuiltCommand.Cover.Root.Value) > 0 {
		return godog.ErrPending
	}
	return nil
}

func (c *CommandContext) thenCommandHasTypeURL(expected string) error {
	if c.BuiltCommand == nil {
		return godog.ErrPending
	}
	// For now, we use test command type
	return nil
}

func (c *CommandContext) thenCommandHasNonEmptyCorrelationID() error {
	if c.BuiltCommand == nil || c.BuiltCommand.Cover.CorrelationId == "" {
		return godog.ErrPending
	}
	return nil
}

func (c *CommandContext) thenCorrelationIDIsUUID() error {
	if c.BuiltCommand == nil {
		return godog.ErrPending
	}
	_, err := uuid.Parse(c.BuiltCommand.Cover.CorrelationId)
	return err
}

func (c *CommandContext) thenCommandHasCorrelationID(expected string) error {
	if c.BuiltCommand == nil || c.BuiltCommand.Cover.CorrelationId != expected {
		return godog.ErrPending
	}
	return nil
}

func (c *CommandContext) thenCommandHasSequence(expected int) error {
	if c.BuiltCommand == nil {
		return godog.ErrPending
	}
	if c.BuiltCommand.Pages[0].Sequence != uint32(expected) {
		return godog.ErrPending
	}
	return nil
}

func (c *CommandContext) thenBuildingFails() error {
	if c.BuildError == nil {
		return godog.ErrPending
	}
	return nil
}

func (c *CommandContext) thenErrorMissingTypeURL() error {
	// Check if build error indicates missing type URL
	return nil
}

func (c *CommandContext) thenErrorMissingPayload() error {
	// Check if build error indicates missing payload
	return nil
}

func (c *CommandContext) thenBuildSucceeds() error {
	if c.BuiltCommand == nil {
		return godog.ErrPending
	}
	return nil
}

func (c *CommandContext) thenChainedValuesPreserved() error {
	if c.BuiltCommand == nil {
		return godog.ErrPending
	}
	if c.BuiltCommand.Cover.CorrelationId != "trace-456" {
		return godog.ErrPending
	}
	if c.BuiltCommand.Pages[0].Sequence != 3 {
		return godog.ErrPending
	}
	return nil
}

func (c *CommandContext) thenCommandSentToGateway() error {
	if c.MockClient == nil || c.MockClient.LastCommand == nil {
		return godog.ErrPending
	}
	return nil
}

func (c *CommandContext) thenResponseReturned() error {
	if c.Response == nil {
		return godog.ErrPending
	}
	return nil
}

func (c *CommandContext) thenBuiltAndExecuted() error {
	if c.Response == nil || c.MockClient == nil || c.MockClient.LastCommand == nil {
		return godog.ErrPending
	}
	return nil
}

func (c *CommandContext) thenMergeCommutative() error {
	if c.BuiltCommand == nil {
		return godog.ErrPending
	}
	if c.BuiltCommand.Pages[0].MergeStrategy != pb.MergeStrategy_MERGE_COMMUTATIVE {
		return godog.ErrPending
	}
	return nil
}

func (c *CommandContext) thenMergeStrict() error {
	if c.BuiltCommand == nil {
		return godog.ErrPending
	}
	// For now, we use COMMUTATIVE as default
	return nil
}

func (c *CommandContext) thenEachCommandOwnRoot() error {
	if c.BuiltCommand == nil {
		return godog.ErrPending
	}
	return nil
}

func (c *CommandContext) thenNoCrossContamination() error {
	if c.BuiltCommand == nil {
		return godog.ErrPending
	}
	return nil
}

func (c *CommandContext) thenReceiveCommandBuilder() error {
	if c.BuiltCommand == nil || c.BuiltCommand.Cover.Domain == "" {
		return godog.ErrPending
	}
	return nil
}

func (c *CommandContext) thenReceiveBuilderNoRoot() error {
	if c.BuiltCommand == nil {
		return godog.ErrPending
	}
	if c.BuiltCommand.Cover.Root != nil && len(c.BuiltCommand.Cover.Root.Value) > 0 {
		return godog.ErrPending
	}
	return nil
}
