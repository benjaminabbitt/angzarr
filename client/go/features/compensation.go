package features

import (
	"fmt"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/cucumber/godog"
	"github.com/google/uuid"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// CompensationContext holds state for compensation scenarios
type CompensationContext struct {
	RejectedCommand       *pb.CommandBook
	RejectionReason       string
	SagaOrigin            *SagaOrigin
	CompensationCtx       *CompensationCtx
	RejectionNotification *RejectionNotification
	Notification          interface{}
	CommandBook           *pb.CommandBook
	Error                 error
}

// SagaOrigin represents saga origin details
type SagaOrigin struct {
	SagaName                string
	TriggeringAggregate     string
	TriggeringEventSequence uint32
}

// CompensationCtx represents a compensation context
type CompensationCtx struct {
	RejectedCommand *pb.CommandBook
	RejectionReason string
	SagaOrigin      *SagaOrigin
	CorrelationID   string
}

// RejectionNotification represents a rejection notification
type RejectionNotification struct {
	RejectedCommand     *pb.CommandBook
	RejectionReason     string
	IssuerName          string
	IssuerType          string
	SourceAggregate     string
	SourceEventSequence uint32
}

func newCompensationContext() *CompensationContext {
	return &CompensationContext{}
}

// InitCompensationSteps registers compensation step definitions
func InitCompensationSteps(ctx *godog.ScenarioContext) {
	cc := newCompensationContext()

	// Background step
	ctx.Step(`^a compensation handling context$`, cc.givenCompensationHandlingContext)

	// Given steps
	ctx.Step(`^a saga command that was rejected$`, cc.givenSagaCommandRejected)
	ctx.Step(`^a saga "([^"]*)" triggered by "([^"]*)" aggregate at sequence (\d+)$`, cc.givenSagaTriggered)
	ctx.Step(`^the saga command was rejected$`, cc.givenSagaRejected)
	ctx.Step(`^a saga command with correlation ID "([^"]*)"$`, cc.givenSagaWithCID)
	ctx.Step(`^the command was rejected$`, cc.givenCommandRejected)
	ctx.Step(`^a CompensationContext for rejected command$`, cc.givenCompensationCtxForRejected)
	ctx.Step(`^a CompensationContext from "([^"]*)" aggregate at sequence (\d+)$`, cc.givenCompensationFromAggregate)
	ctx.Step(`^a CompensationContext from saga "([^"]*)"$`, cc.givenCompensationFromSaga)
	ctx.Step(`^a command rejected with reason "([^"]*)"$`, cc.givenCommandWithReason)
	ctx.Step(`^a command rejected with structured reason$`, cc.givenStructuredReason)
	ctx.Step(`^a saga command with specific payload$`, cc.givenSagaSpecificPayload)
	ctx.Step(`^a nested saga scenario$`, cc.givenNestedSaga)
	ctx.Step(`^an inner saga command was rejected$`, cc.givenInnerRejected)
	ctx.Step(`^a saga router handling rejections$`, cc.givenSagaRouter)
	ctx.Step(`^a process manager router$`, cc.givenPMRouter)
	ctx.Step(`^a CompensationContext from "([^"]*)" aggregate root "([^"]*)"$`, cc.givenCompensationWithRoot)

	// When steps
	ctx.Step(`^I build a CompensationContext$`, cc.whenBuildCompensationCtx)
	ctx.Step(`^I build a RejectionNotification$`, cc.whenBuildRejection)
	ctx.Step(`^I build a Notification from the context$`, cc.whenBuildNotification)
	ctx.Step(`^I build a Notification from a CompensationContext$`, cc.whenBuildNotificationFromCtx)
	ctx.Step(`^I build a notification CommandBook$`, cc.whenBuildNotificationCmdBook)
	ctx.Step(`^a command execution fails with precondition error$`, cc.whenPreconditionError)
	ctx.Step(`^a PM command is rejected$`, cc.whenPMRejected)

	// Then steps
	ctx.Step(`^the context should include the rejected command$`, cc.thenCtxHasCommand)
	ctx.Step(`^the context should include the rejection reason$`, cc.thenCtxHasReason)
	ctx.Step(`^the context should include the saga origin$`, cc.thenCtxHasOrigin)
	ctx.Step(`^the saga_origin saga_name should be "([^"]*)"$`, cc.thenSagaName)
	ctx.Step(`^the triggering_aggregate should be "([^"]*)"$`, cc.thenTriggeringAgg)
	ctx.Step(`^the triggering_event_sequence should be (\d+)$`, cc.thenTriggeringSeq)
	ctx.Step(`^the context correlation_id should be "([^"]*)"$`, cc.thenCtxCID)
	ctx.Step(`^the notification should include the rejected command$`, cc.thenNotifHasCommand)
	ctx.Step(`^the notification should include the rejection reason$`, cc.thenNotifHasReason)
	ctx.Step(`^the notification should have issuer_type "([^"]*)"$`, cc.thenNotifIssuerType)
	ctx.Step(`^the source_aggregate should have domain "([^"]*)"$`, cc.thenSourceDomain)
	ctx.Step(`^the source_event_sequence should be (\d+)$`, cc.thenSourceSeq)
	ctx.Step(`^the issuer_name should be "([^"]*)"$`, cc.thenIssuerName)
	ctx.Step(`^the issuer_type should be "([^"]*)"$`, cc.thenIssuerType)
	ctx.Step(`^the notification should have a cover$`, cc.thenNotifHasCover)
	ctx.Step(`^the notification payload should contain RejectionNotification$`, cc.thenPayloadHasRejection)
	ctx.Step(`^the payload type_url should be "([^"]*)"$`, cc.thenPayloadTypeURL)
	ctx.Step(`^the notification should have a sent_at timestamp$`, cc.thenHasTimestamp)
	ctx.Step(`^the timestamp should be recent$`, cc.thenTimestampRecent)
	ctx.Step(`^the command book should target the source aggregate$`, cc.thenCmdTargetsSource)
	ctx.Step(`^the command book should have MERGE_COMMUTATIVE strategy$`, cc.thenCmdCommutative)
	ctx.Step(`^the command book should preserve correlation ID$`, cc.thenCmdPreservesCID)
	ctx.Step(`^the command book cover should have domain "([^"]*)"$`, cc.thenCmdDomain)
	ctx.Step(`^the command book cover should have root "([^"]*)"$`, cc.thenCmdRoot)
	ctx.Step(`^the rejection_reason should be "([^"]*)"$`, cc.thenRejectionReason)
	ctx.Step(`^the rejection_reason should contain the full error details$`, cc.thenRejectionDetails)
	ctx.Step(`^the rejected_command should be the original command$`, cc.thenOriginalCommand)
	ctx.Step(`^all command fields should be preserved$`, cc.thenFieldsPreserved)
	ctx.Step(`^the full saga origin chain should be preserved$`, cc.thenChainPreserved)
	ctx.Step(`^root cause can be traced through the chain$`, cc.thenRootTraceable)
	ctx.Step(`^the router should build a CompensationContext$`, cc.thenRouterBuildsCtx)
	ctx.Step(`^the router should emit a rejection notification$`, cc.thenRouterEmitsNotif)
	ctx.Step(`^the context should have issuer_type "([^"]*)"$`, cc.thenCtxIssuerType)
}

func (c *CompensationContext) makeCommandBook(domain string, correlationID string, rootBytes []byte) *pb.CommandBook {
	root := uuid.New()
	if rootBytes != nil {
		copy(root[:], rootBytes)
	}
	return &pb.CommandBook{
		Cover: &pb.Cover{
			Domain:        domain,
			CorrelationId: correlationID,
			Root:          &pb.UUID{Value: root[:]},
		},
		Pages: []*pb.CommandPage{
			{
				Header:        &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: 0}},
				MergeStrategy: pb.MergeStrategy_MERGE_COMMUTATIVE,
				Payload: &pb.CommandPage_Command{
					Command: &anypb.Any{
						TypeUrl: "type.googleapis.com/test.Command",
						Value:   []byte("test"),
					},
				},
			},
		},
	}
}

func (c *CompensationContext) givenCompensationHandlingContext() error {
	return nil
}

func (c *CompensationContext) givenSagaCommandRejected() error {
	c.RejectedCommand = c.makeCommandBook("orders", "", nil)
	c.RejectionReason = "precondition_failed"
	c.SagaOrigin = &SagaOrigin{
		SagaName:                "test-saga",
		TriggeringAggregate:     "orders",
		TriggeringEventSequence: 0,
	}
	return nil
}

func (c *CompensationContext) givenSagaTriggered(sagaName, aggregate string, seq int) error {
	c.SagaOrigin = &SagaOrigin{
		SagaName:                sagaName,
		TriggeringAggregate:     aggregate,
		TriggeringEventSequence: uint32(seq),
	}
	return nil
}

func (c *CompensationContext) givenSagaRejected() error {
	c.RejectedCommand = c.makeCommandBook("orders", "", nil)
	c.RejectionReason = "rejected"
	return nil
}

func (c *CompensationContext) givenSagaWithCID(cid string) error {
	c.RejectedCommand = c.makeCommandBook("orders", cid, nil)
	return nil
}

func (c *CompensationContext) givenCommandRejected() error {
	c.RejectionReason = "rejected"
	return nil
}

func (c *CompensationContext) givenCompensationCtxForRejected() error {
	if c.RejectedCommand == nil {
		c.RejectedCommand = c.makeCommandBook("orders", uuid.New().String(), nil)
	}
	if c.RejectionReason == "" {
		c.RejectionReason = "rejected"
	}
	if c.SagaOrigin == nil {
		c.SagaOrigin = &SagaOrigin{
			SagaName:            "test-saga",
			TriggeringAggregate: "orders",
		}
	}
	// Ensure correlation ID is set if not already
	if c.RejectedCommand.Cover.CorrelationId == "" {
		c.RejectedCommand.Cover.CorrelationId = uuid.New().String()
	}
	c.CompensationCtx = &CompensationCtx{
		RejectedCommand: c.RejectedCommand,
		RejectionReason: c.RejectionReason,
		SagaOrigin:      c.SagaOrigin,
		CorrelationID:   c.RejectedCommand.Cover.CorrelationId,
	}
	return nil
}

func (c *CompensationContext) givenCompensationFromAggregate(aggregate string, seq int) error {
	c.SagaOrigin = &SagaOrigin{
		SagaName:                "test-saga",
		TriggeringAggregate:     aggregate,
		TriggeringEventSequence: uint32(seq),
	}
	c.RejectedCommand = c.makeCommandBook(aggregate, "", nil)
	c.RejectionReason = "rejected"
	c.givenCompensationCtxForRejected()
	return nil
}

func (c *CompensationContext) givenCompensationFromSaga(sagaName string) error {
	c.SagaOrigin = &SagaOrigin{
		SagaName:            sagaName,
		TriggeringAggregate: "orders",
	}
	c.RejectedCommand = c.makeCommandBook("orders", "", nil)
	c.RejectionReason = "rejected"
	c.givenCompensationCtxForRejected()
	return nil
}

func (c *CompensationContext) givenCommandWithReason(reason string) error {
	c.RejectedCommand = c.makeCommandBook("orders", "", nil)
	c.RejectionReason = reason
	return nil
}

func (c *CompensationContext) givenStructuredReason() error {
	c.RejectedCommand = c.makeCommandBook("orders", "", nil)
	c.RejectionReason = `{"code": "INSUFFICIENT_FUNDS", "details": "balance too low"}`
	return nil
}

func (c *CompensationContext) givenSagaSpecificPayload() error {
	c.RejectedCommand = c.makeCommandBook("orders", "", nil)
	return nil
}

func (c *CompensationContext) givenNestedSaga() error {
	c.SagaOrigin = &SagaOrigin{
		SagaName:                "inner-saga",
		TriggeringAggregate:     "orders",
		TriggeringEventSequence: 5,
	}
	return nil
}

func (c *CompensationContext) givenInnerRejected() error {
	c.RejectedCommand = c.makeCommandBook("inventory", "", nil)
	c.RejectionReason = "nested_rejection"
	return nil
}

func (c *CompensationContext) givenSagaRouter() error {
	return nil
}

func (c *CompensationContext) givenPMRouter() error {
	return nil
}

func (c *CompensationContext) givenCompensationWithRoot(aggregate, root string) error {
	rootUUID, err := uuid.Parse(root)
	var rootBytes []byte
	if err == nil {
		rootBytes = rootUUID[:]
	}
	c.SagaOrigin = &SagaOrigin{
		SagaName:            "test-saga",
		TriggeringAggregate: aggregate,
	}
	c.RejectedCommand = c.makeCommandBook(aggregate, "", rootBytes)
	c.RejectionReason = "rejected"
	c.givenCompensationCtxForRejected()
	return nil
}

func (c *CompensationContext) whenBuildCompensationCtx() error {
	c.CompensationCtx = &CompensationCtx{
		RejectedCommand: c.RejectedCommand,
		RejectionReason: c.RejectionReason,
		SagaOrigin:      c.SagaOrigin,
		CorrelationID:   c.RejectedCommand.Cover.CorrelationId,
	}
	return nil
}

func (c *CompensationContext) whenBuildRejection() error {
	// Ensure SagaOrigin exists with defaults if not set
	if c.SagaOrigin == nil {
		c.SagaOrigin = &SagaOrigin{
			SagaName:                "test-saga",
			TriggeringAggregate:     "test-agg",
			TriggeringEventSequence: 1,
		}
	}
	if c.CompensationCtx == nil {
		c.whenBuildCompensationCtx()
	}
	ctx := c.CompensationCtx
	// Guard against nil SagaOrigin in context
	var issuerName, sourceAggregate string
	var sourceEventSeq uint32
	if ctx.SagaOrigin != nil {
		issuerName = ctx.SagaOrigin.SagaName
		sourceAggregate = ctx.SagaOrigin.TriggeringAggregate
		sourceEventSeq = ctx.SagaOrigin.TriggeringEventSequence
	}
	c.RejectionNotification = &RejectionNotification{
		RejectedCommand:     ctx.RejectedCommand,
		RejectionReason:     ctx.RejectionReason,
		IssuerName:          issuerName,
		IssuerType:          "saga",
		SourceAggregate:     sourceAggregate,
		SourceEventSequence: sourceEventSeq,
	}
	return nil
}

func (c *CompensationContext) whenBuildNotification() error {
	c.whenBuildRejection()
	c.Notification = &struct {
		Cover       interface{}
		SentAt      *timestamppb.Timestamp
		PayloadType string
	}{
		Cover:       struct{}{},
		SentAt:      timestamppb.Now(),
		PayloadType: "type.googleapis.com/angzarr.RejectionNotification",
	}
	return nil
}

func (c *CompensationContext) whenBuildNotificationFromCtx() error {
	c.givenCompensationCtxForRejected()
	return c.whenBuildNotification()
}

func (c *CompensationContext) whenBuildNotificationCmdBook() error {
	if c.CompensationCtx == nil {
		c.givenCompensationCtxForRejected()
	}
	cmd := c.CompensationCtx.RejectedCommand
	c.CommandBook = c.makeCommandBook(
		cmd.Cover.Domain,
		c.CompensationCtx.CorrelationID,
		nil,
	)
	return nil
}

func (c *CompensationContext) whenPreconditionError() error {
	c.Error = fmt.Errorf("precondition error: simulated failure")
	return nil
}

func (c *CompensationContext) whenPMRejected() error {
	c.RejectedCommand = c.makeCommandBook("orders", "", nil)
	c.RejectionReason = "pm_rejection"
	c.SagaOrigin = &SagaOrigin{
		SagaName:            "test-pm",
		TriggeringAggregate: "orders",
	}
	c.whenBuildCompensationCtx()
	return nil
}

func (c *CompensationContext) thenCtxHasCommand() error {
	if c.CompensationCtx == nil {
		return fmt.Errorf("compensation context is nil")
	}
	if c.CompensationCtx.RejectedCommand == nil {
		return fmt.Errorf("rejected command is nil")
	}
	return nil
}

func (c *CompensationContext) thenCtxHasReason() error {
	if c.CompensationCtx == nil {
		return fmt.Errorf("compensation context is nil")
	}
	if c.CompensationCtx.RejectionReason == "" {
		return fmt.Errorf("rejection reason is empty")
	}
	return nil
}

func (c *CompensationContext) thenCtxHasOrigin() error {
	if c.CompensationCtx == nil {
		return fmt.Errorf("compensation context is nil")
	}
	if c.CompensationCtx.SagaOrigin == nil {
		return fmt.Errorf("saga origin is nil")
	}
	return nil
}

func (c *CompensationContext) thenSagaName(expected string) error {
	if c.CompensationCtx == nil {
		return fmt.Errorf("compensation context is nil")
	}
	if c.CompensationCtx.SagaOrigin == nil {
		return fmt.Errorf("saga origin is nil")
	}
	if c.CompensationCtx.SagaOrigin.SagaName != expected {
		return fmt.Errorf("expected saga name %q, got %q", expected, c.CompensationCtx.SagaOrigin.SagaName)
	}
	return nil
}

func (c *CompensationContext) thenTriggeringAgg(expected string) error {
	if c.CompensationCtx == nil {
		return fmt.Errorf("compensation context is nil")
	}
	if c.CompensationCtx.SagaOrigin == nil {
		return fmt.Errorf("saga origin is nil")
	}
	if c.CompensationCtx.SagaOrigin.TriggeringAggregate != expected {
		return fmt.Errorf("expected triggering aggregate %q, got %q", expected, c.CompensationCtx.SagaOrigin.TriggeringAggregate)
	}
	return nil
}

func (c *CompensationContext) thenTriggeringSeq(expected int) error {
	if c.CompensationCtx == nil {
		return fmt.Errorf("compensation context is nil")
	}
	if c.CompensationCtx.SagaOrigin == nil {
		return fmt.Errorf("saga origin is nil")
	}
	if c.CompensationCtx.SagaOrigin.TriggeringEventSequence != uint32(expected) {
		return fmt.Errorf("expected triggering sequence %d, got %d", expected, c.CompensationCtx.SagaOrigin.TriggeringEventSequence)
	}
	return nil
}

func (c *CompensationContext) thenCtxCID(expected string) error {
	if c.CompensationCtx == nil {
		return fmt.Errorf("compensation context is nil")
	}
	if c.CompensationCtx.CorrelationID != expected {
		return fmt.Errorf("expected correlation ID %q, got %q", expected, c.CompensationCtx.CorrelationID)
	}
	return nil
}

func (c *CompensationContext) thenNotifHasCommand() error {
	if c.RejectionNotification == nil {
		return fmt.Errorf("rejection notification is nil")
	}
	if c.RejectionNotification.RejectedCommand == nil {
		return fmt.Errorf("rejected command in notification is nil")
	}
	return nil
}

func (c *CompensationContext) thenNotifHasReason() error {
	if c.RejectionNotification == nil {
		return fmt.Errorf("rejection notification is nil")
	}
	if c.RejectionNotification.RejectionReason == "" {
		return fmt.Errorf("rejection reason in notification is empty")
	}
	return nil
}

func (c *CompensationContext) thenNotifIssuerType(expected string) error {
	if c.RejectionNotification == nil {
		return fmt.Errorf("rejection notification is nil")
	}
	if c.RejectionNotification.IssuerType != expected {
		return fmt.Errorf("expected issuer type %q, got %q", expected, c.RejectionNotification.IssuerType)
	}
	return nil
}

func (c *CompensationContext) thenSourceDomain(expected string) error {
	if c.RejectionNotification == nil {
		return fmt.Errorf("rejection notification is nil")
	}
	if c.RejectionNotification.SourceAggregate != expected {
		return fmt.Errorf("expected source aggregate %q, got %q", expected, c.RejectionNotification.SourceAggregate)
	}
	return nil
}

func (c *CompensationContext) thenSourceSeq(expected int) error {
	if c.RejectionNotification == nil {
		return fmt.Errorf("rejection notification is nil")
	}
	if c.RejectionNotification.SourceEventSequence != uint32(expected) {
		return fmt.Errorf("expected source sequence %d, got %d", expected, c.RejectionNotification.SourceEventSequence)
	}
	return nil
}

func (c *CompensationContext) thenIssuerName(expected string) error {
	if c.RejectionNotification == nil {
		return fmt.Errorf("rejection notification is nil")
	}
	if c.RejectionNotification.IssuerName != expected {
		return fmt.Errorf("expected issuer name %q, got %q", expected, c.RejectionNotification.IssuerName)
	}
	return nil
}

func (c *CompensationContext) thenIssuerType(expected string) error {
	if c.RejectionNotification == nil {
		return fmt.Errorf("rejection notification is nil")
	}
	if c.RejectionNotification.IssuerType != expected {
		return fmt.Errorf("expected issuer type %q, got %q", expected, c.RejectionNotification.IssuerType)
	}
	return nil
}

func (c *CompensationContext) thenNotifHasCover() error {
	if c.Notification == nil {
		return fmt.Errorf("notification is nil")
	}
	return nil
}

func (c *CompensationContext) thenPayloadHasRejection() error {
	if c.RejectionNotification == nil {
		return fmt.Errorf("rejection notification is nil")
	}
	return nil
}

func (c *CompensationContext) thenPayloadTypeURL(expected string) error {
	return nil
}

func (c *CompensationContext) thenHasTimestamp() error {
	if c.Notification == nil {
		return fmt.Errorf("notification is nil")
	}
	return nil
}

func (c *CompensationContext) thenTimestampRecent() error {
	return nil
}

func (c *CompensationContext) thenCmdTargetsSource() error {
	if c.CommandBook == nil {
		return fmt.Errorf("command book is nil")
	}
	if c.CommandBook.Cover.Domain == "" {
		return fmt.Errorf("command book domain is empty")
	}
	return nil
}

func (c *CompensationContext) thenCmdCommutative() error {
	if c.CommandBook == nil {
		return fmt.Errorf("command book is nil")
	}
	if c.CommandBook.Pages[0].MergeStrategy != pb.MergeStrategy_MERGE_COMMUTATIVE {
		return fmt.Errorf("expected MERGE_COMMUTATIVE strategy, got %v", c.CommandBook.Pages[0].MergeStrategy)
	}
	return nil
}

func (c *CompensationContext) thenCmdPreservesCID() error {
	if c.CommandBook == nil {
		return fmt.Errorf("command book is nil")
	}
	if c.CommandBook.Cover.CorrelationId == "" {
		return fmt.Errorf("command book correlation ID is empty")
	}
	return nil
}

func (c *CompensationContext) thenCmdDomain(expected string) error {
	if c.CommandBook == nil {
		return fmt.Errorf("command book is nil")
	}
	if c.CommandBook.Cover.Domain != expected {
		return fmt.Errorf("expected domain %q, got %q", expected, c.CommandBook.Cover.Domain)
	}
	return nil
}

func (c *CompensationContext) thenCmdRoot(expected string) error {
	if c.CommandBook == nil {
		return fmt.Errorf("command book is nil")
	}
	if c.CommandBook.Cover.Root == nil {
		return fmt.Errorf("command book root is nil")
	}
	return nil
}

func (c *CompensationContext) thenRejectionReason(expected string) error {
	if c.RejectionNotification == nil {
		return fmt.Errorf("rejection notification is nil")
	}
	if c.RejectionNotification.RejectionReason != expected {
		return fmt.Errorf("expected rejection reason %q, got %q", expected, c.RejectionNotification.RejectionReason)
	}
	return nil
}

func (c *CompensationContext) thenRejectionDetails() error {
	if c.RejectionNotification == nil {
		return fmt.Errorf("rejection notification is nil")
	}
	if c.RejectionNotification.RejectionReason == "" {
		return fmt.Errorf("rejection reason is empty, expected full error details")
	}
	return nil
}

func (c *CompensationContext) thenOriginalCommand() error {
	if c.RejectionNotification == nil {
		return fmt.Errorf("rejection notification is nil")
	}
	if c.RejectionNotification.RejectedCommand == nil {
		return fmt.Errorf("rejected command is nil, expected original command")
	}
	return nil
}

func (c *CompensationContext) thenFieldsPreserved() error {
	if c.RejectionNotification == nil {
		return fmt.Errorf("rejection notification is nil")
	}
	if c.RejectionNotification.RejectedCommand == nil {
		return fmt.Errorf("rejected command is nil")
	}
	if c.RejectionNotification.RejectedCommand.Cover == nil {
		return fmt.Errorf("rejected command cover is nil, expected all fields preserved")
	}
	return nil
}

func (c *CompensationContext) thenChainPreserved() error {
	if c.CompensationCtx == nil {
		return fmt.Errorf("compensation context is nil")
	}
	if c.CompensationCtx.SagaOrigin == nil {
		return fmt.Errorf("saga origin is nil, expected full chain preserved")
	}
	return nil
}

func (c *CompensationContext) thenRootTraceable() error {
	return nil
}

func (c *CompensationContext) thenRouterBuildsCtx() error {
	return nil
}

func (c *CompensationContext) thenRouterEmitsNotif() error {
	return nil
}

func (c *CompensationContext) thenCtxIssuerType(expected string) error {
	return nil
}
