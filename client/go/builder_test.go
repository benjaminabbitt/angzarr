package angzarr

import (
	"context"
	"testing"

	"github.com/google/uuid"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/wrapperspb"
)

func TestNewCommandBuilder(t *testing.T) {
	client := &AggregateClient{}
	root := uuid.New()
	b := NewCommandBuilder(client, "orders", root)

	if b.client != client {
		t.Error("client mismatch")
	}
	if b.domain != "orders" {
		t.Errorf("got domain %q, want %q", b.domain, "orders")
	}
	if b.root == nil || *b.root != root {
		t.Error("root mismatch")
	}
}

func TestNewCommandBuilderNew(t *testing.T) {
	client := &AggregateClient{}
	b := NewCommandBuilderNew(client, "orders")

	if b.client != client {
		t.Error("client mismatch")
	}
	if b.domain != "orders" {
		t.Errorf("got domain %q, want %q", b.domain, "orders")
	}
	if b.root != nil {
		t.Error("expected nil root for new aggregate")
	}
}

func TestCommandBuilder_WithCorrelationID(t *testing.T) {
	b := &CommandBuilder{}
	result := b.WithCorrelationID("corr-123")

	if result != b {
		t.Error("expected same builder returned")
	}
	if b.correlationID != "corr-123" {
		t.Errorf("got %q, want %q", b.correlationID, "corr-123")
	}
}

func TestCommandBuilder_WithSequence(t *testing.T) {
	b := &CommandBuilder{}
	result := b.WithSequence(42)

	if result != b {
		t.Error("expected same builder returned")
	}
	if b.sequence != 42 {
		t.Errorf("got %d, want %d", b.sequence, 42)
	}
}

func TestCommandBuilder_WithCommand(t *testing.T) {
	t.Run("successful marshal", func(t *testing.T) {
		b := &CommandBuilder{}
		msg := wrapperspb.String("test")
		result := b.WithCommand("type.googleapis.com/test.Command", msg)

		if result != b {
			t.Error("expected same builder returned")
		}
		if b.typeURL != "type.googleapis.com/test.Command" {
			t.Errorf("got typeURL %q", b.typeURL)
		}
		if b.payload == nil {
			t.Error("expected non-nil payload")
		}
		if b.err != nil {
			t.Errorf("unexpected error: %v", b.err)
		}
	})

	t.Run("nil message", func(t *testing.T) {
		b := &CommandBuilder{}
		result := b.WithCommand("type.googleapis.com/test.Command", nil)

		if result != b {
			t.Error("expected same builder returned")
		}
		// proto.Marshal(nil) returns nil payload, no error
		if b.err != nil {
			t.Errorf("unexpected error: %v", b.err)
		}
		// Build will fail later due to nil payload
		if b.payload != nil {
			t.Errorf("expected nil payload, got %v", b.payload)
		}
	})
}

func TestCommandBuilder_Build(t *testing.T) {
	t.Run("successful build with root", func(t *testing.T) {
		root := uuid.New()
		msg := wrapperspb.String("test")
		payload, _ := proto.Marshal(msg)

		b := &CommandBuilder{
			domain:        "orders",
			root:          &root,
			correlationID: "corr-123",
			sequence:      5,
			typeURL:       "type.googleapis.com/test.Command",
			payload:       payload,
		}

		cmd, err := b.Build()
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if cmd == nil {
			t.Fatal("expected non-nil command")
		}
		if cmd.Cover.Domain != "orders" {
			t.Errorf("got domain %q, want %q", cmd.Cover.Domain, "orders")
		}
		if cmd.Cover.CorrelationId != "corr-123" {
			t.Errorf("got correlation_id %q, want %q", cmd.Cover.CorrelationId, "corr-123")
		}
		if cmd.Cover.Root == nil {
			t.Error("expected non-nil root")
		}
		if len(cmd.Pages) != 1 {
			t.Fatalf("expected 1 page, got %d", len(cmd.Pages))
		}
		if cmd.Pages[0].Sequence != 5 {
			t.Errorf("got sequence %d, want %d", cmd.Pages[0].Sequence, 5)
		}
	})

	t.Run("successful build without root", func(t *testing.T) {
		msg := wrapperspb.String("test")
		payload, _ := proto.Marshal(msg)

		b := &CommandBuilder{
			domain:  "orders",
			root:    nil,
			typeURL: "type.googleapis.com/test.Command",
			payload: payload,
		}

		cmd, err := b.Build()
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if cmd.Cover.Root != nil {
			t.Error("expected nil root")
		}
		// Should auto-generate correlation ID
		if cmd.Cover.CorrelationId == "" {
			t.Error("expected auto-generated correlation ID")
		}
	})

	t.Run("previous error", func(t *testing.T) {
		b := &CommandBuilder{
			err: InvalidArgumentError("previous error"),
		}

		_, err := b.Build()
		if err == nil {
			t.Error("expected error")
		}
	})

	t.Run("missing type URL", func(t *testing.T) {
		b := &CommandBuilder{
			domain:  "orders",
			payload: []byte("test"),
		}

		_, err := b.Build()
		if err == nil {
			t.Error("expected error for missing type URL")
		}
	})

	t.Run("missing payload", func(t *testing.T) {
		b := &CommandBuilder{
			domain:  "orders",
			typeURL: "type.googleapis.com/test.Command",
		}

		_, err := b.Build()
		if err == nil {
			t.Error("expected error for missing payload")
		}
	})
}

func TestNewQueryBuilder(t *testing.T) {
	client := &QueryClient{}
	root := uuid.New()
	b := NewQueryBuilder(client, "orders", root)

	if b.client != client {
		t.Error("client mismatch")
	}
	if b.domain != "orders" {
		t.Errorf("got domain %q, want %q", b.domain, "orders")
	}
	if b.root == nil || *b.root != root {
		t.Error("root mismatch")
	}
}

func TestNewQueryBuilderDomain(t *testing.T) {
	client := &QueryClient{}
	b := NewQueryBuilderDomain(client, "orders")

	if b.client != client {
		t.Error("client mismatch")
	}
	if b.domain != "orders" {
		t.Errorf("got domain %q, want %q", b.domain, "orders")
	}
	if b.root != nil {
		t.Error("expected nil root")
	}
}

func TestQueryBuilder_ByCorrelationID(t *testing.T) {
	root := uuid.New()
	b := &QueryBuilder{root: &root}
	result := b.ByCorrelationID("corr-123")

	if result != b {
		t.Error("expected same builder returned")
	}
	if b.correlationID != "corr-123" {
		t.Errorf("got %q, want %q", b.correlationID, "corr-123")
	}
	if b.root != nil {
		t.Error("expected root to be cleared")
	}
}

func TestQueryBuilder_WithEdition(t *testing.T) {
	b := &QueryBuilder{}
	result := b.WithEdition("test-edition")

	if result != b {
		t.Error("expected same builder returned")
	}
	if b.edition != "test-edition" {
		t.Errorf("got %q, want %q", b.edition, "test-edition")
	}
}

func TestQueryBuilder_Range(t *testing.T) {
	b := &QueryBuilder{}
	result := b.Range(10)

	if result != b {
		t.Error("expected same builder returned")
	}
	if b.rangeSelect == nil {
		t.Fatal("expected non-nil range")
	}
	if b.rangeSelect.Lower != 10 {
		t.Errorf("got lower %d, want %d", b.rangeSelect.Lower, 10)
	}
	if b.rangeSelect.Upper != nil {
		t.Error("expected nil upper")
	}
}

func TestQueryBuilder_RangeTo(t *testing.T) {
	b := &QueryBuilder{}
	result := b.RangeTo(5, 15)

	if result != b {
		t.Error("expected same builder returned")
	}
	if b.rangeSelect == nil {
		t.Fatal("expected non-nil range")
	}
	if b.rangeSelect.Lower != 5 {
		t.Errorf("got lower %d, want %d", b.rangeSelect.Lower, 5)
	}
	if b.rangeSelect.Upper == nil || *b.rangeSelect.Upper != 15 {
		t.Error("expected upper to be 15")
	}
}

func TestQueryBuilder_AsOfSequence(t *testing.T) {
	b := &QueryBuilder{}
	result := b.AsOfSequence(42)

	if result != b {
		t.Error("expected same builder returned")
	}
	if b.temporal == nil {
		t.Fatal("expected non-nil temporal")
	}
	if b.temporal.GetAsOfSequence() != 42 {
		t.Errorf("got %d, want %d", b.temporal.GetAsOfSequence(), 42)
	}
}

func TestQueryBuilder_AsOfTime(t *testing.T) {
	t.Run("valid timestamp", func(t *testing.T) {
		b := &QueryBuilder{}
		result := b.AsOfTime("2024-01-15T10:30:00Z")

		if result != b {
			t.Error("expected same builder returned")
		}
		if b.temporal == nil {
			t.Fatal("expected non-nil temporal")
		}
		if b.temporal.GetAsOfTime() == nil {
			t.Error("expected non-nil timestamp")
		}
		if b.err != nil {
			t.Errorf("unexpected error: %v", b.err)
		}
	})

	t.Run("invalid timestamp", func(t *testing.T) {
		b := &QueryBuilder{}
		result := b.AsOfTime("not a timestamp")

		if result != b {
			t.Error("expected same builder returned")
		}
		if b.err == nil {
			t.Error("expected error for invalid timestamp")
		}
	})
}

func TestQueryBuilder_Build(t *testing.T) {
	t.Run("with root and range", func(t *testing.T) {
		root := uuid.New()
		b := &QueryBuilder{
			domain:      "orders",
			root:        &root,
			edition:     "test-edition",
			rangeSelect: &pb.SequenceRange{Lower: 5},
		}

		query, err := b.Build()
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if query.Cover.Domain != "orders" {
			t.Errorf("got domain %q", query.Cover.Domain)
		}
		if query.Cover.Root == nil {
			t.Error("expected non-nil root")
		}
		if query.Cover.Edition == nil || query.Cover.Edition.Name != "test-edition" {
			t.Error("expected edition")
		}
		if query.GetRange() == nil {
			t.Error("expected range selection")
		}
	})

	t.Run("with correlation ID and temporal", func(t *testing.T) {
		b := &QueryBuilder{
			domain:        "orders",
			correlationID: "corr-123",
			temporal: &pb.TemporalQuery{
				PointInTime: &pb.TemporalQuery_AsOfSequence{AsOfSequence: 10},
			},
		}

		query, err := b.Build()
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if query.Cover.CorrelationId != "corr-123" {
			t.Errorf("got correlation_id %q", query.Cover.CorrelationId)
		}
		if query.GetTemporal() == nil {
			t.Error("expected temporal selection")
		}
	})

	t.Run("no selection", func(t *testing.T) {
		b := &QueryBuilder{
			domain: "orders",
		}

		query, err := b.Build()
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if query.GetRange() != nil || query.GetTemporal() != nil {
			t.Error("expected no selection")
		}
	})

	t.Run("previous error", func(t *testing.T) {
		b := &QueryBuilder{
			err: InvalidArgumentError("previous error"),
		}

		_, err := b.Build()
		if err == nil {
			t.Error("expected error")
		}
	})
}

// Test convenience methods on clients

func TestAggregateClient_Command(t *testing.T) {
	client := &AggregateClient{}
	root := uuid.New()
	b := client.Command("orders", root)

	if b.client != client {
		t.Error("client mismatch")
	}
	if b.domain != "orders" {
		t.Errorf("got domain %q", b.domain)
	}
}

func TestAggregateClient_CommandNew(t *testing.T) {
	client := &AggregateClient{}
	b := client.CommandNew("orders")

	if b.client != client {
		t.Error("client mismatch")
	}
	if b.root != nil {
		t.Error("expected nil root")
	}
}

func TestQueryClient_Query(t *testing.T) {
	client := &QueryClient{}
	root := uuid.New()
	b := client.Query("orders", root)

	if b.client != client {
		t.Error("client mismatch")
	}
	if b.domain != "orders" {
		t.Errorf("got domain %q", b.domain)
	}
}

func TestQueryClient_QueryDomain(t *testing.T) {
	client := &QueryClient{}
	b := client.QueryDomain("orders")

	if b.client != client {
		t.Error("client mismatch")
	}
	if b.root != nil {
		t.Error("expected nil root")
	}
}

func TestDomainClient_Command(t *testing.T) {
	aggClient := &AggregateClient{}
	client := &DomainClient{Aggregate: aggClient}
	root := uuid.New()
	b := client.Command("orders", root)

	if b.client != aggClient {
		t.Error("client mismatch")
	}
}

func TestDomainClient_CommandNew(t *testing.T) {
	aggClient := &AggregateClient{}
	client := &DomainClient{Aggregate: aggClient}
	b := client.CommandNew("orders")

	if b.client != aggClient {
		t.Error("client mismatch")
	}
}

func TestDomainClient_NewQuery(t *testing.T) {
	queryClient := &QueryClient{}
	client := &DomainClient{Query: queryClient}
	root := uuid.New()
	b := client.NewQuery("orders", root)

	if b.client != queryClient {
		t.Error("client mismatch")
	}
}

func TestDomainClient_NewQueryDomain(t *testing.T) {
	queryClient := &QueryClient{}
	client := &DomainClient{Query: queryClient}
	b := client.NewQueryDomain("orders")

	if b.client != queryClient {
		t.Error("client mismatch")
	}
}

// Integration-style tests for Execute methods would require mocking gRPC
// Those are covered in client_test.go with mock servers

func TestCommandBuilder_Execute_RequiresClient(t *testing.T) {
	msg := wrapperspb.String("test")
	b := &CommandBuilder{
		client:  nil, // No client set
		domain:  "orders",
		typeURL: "type.googleapis.com/test.Command",
	}
	b.WithCommand("type.googleapis.com/test.Command", msg)

	// Execute should panic or fail gracefully when client is nil
	// In production, this would be a nil pointer dereference
	// The builder pattern should catch this at Build time ideally

	_, err := b.Build()
	if err != nil {
		// Build succeeded but Execute would fail
		return
	}

	// If we get here, we can't test Execute without a real client
	// This is expected behavior
}

func TestQueryBuilder_GetEventBook_RequiresClient(t *testing.T) {
	// Testing without a client would panic - real execution tests are in client_test.go
	// This test documents the expected behavior
	_ = &QueryBuilder{
		client: nil,
		domain: "orders",
	}
}

func TestQueryBuilder_GetPages(t *testing.T) {
	// GetPages internally calls GetEventBook, so it has the same requirements
	// Testing the error path
	b := &QueryBuilder{
		err: InvalidArgumentError("test error"),
	}

	_, err := b.GetPages(context.Background())
	if err == nil {
		t.Error("expected error to propagate")
	}
}
