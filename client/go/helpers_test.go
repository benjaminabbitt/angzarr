package angzarr

import (
	"testing"
	"time"

	"github.com/google/uuid"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

func TestConstants(t *testing.T) {
	tests := []struct {
		name  string
		value string
		want  string
	}{
		{"UnknownDomain", UnknownDomain, "unknown"},
		{"WildcardDomain", WildcardDomain, "*"},
		{"DefaultEdition", DefaultEdition, "angzarr"},
		{"MetaAngzarrDomain", MetaAngzarrDomain, "_angzarr"},
		{"ProjectionDomainPrefix", ProjectionDomainPrefix, "projection:"},
		{"CorrelationIDHeader", CorrelationIDHeader, "x-correlation-id"},
		{"TypeURLPrefix", TypeURLPrefix, "type.googleapis.com/"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if tt.value != tt.want {
				t.Errorf("got %q, want %q", tt.value, tt.want)
			}
		})
	}
}

func TestCoverOf(t *testing.T) {
	cover := &pb.Cover{Domain: "test"}

	tests := []struct {
		name  string
		input interface{}
		want  *pb.Cover
	}{
		{"EventBook", &pb.EventBook{Cover: cover}, cover},
		{"CommandBook", &pb.CommandBook{Cover: cover}, cover},
		{"Query", &pb.Query{Cover: cover}, cover},
		{"Cover directly", cover, cover},
		{"nil EventBook", (*pb.EventBook)(nil), nil},
		{"nil Cover", (*pb.Cover)(nil), nil},
		{"unknown type", "not a proto", nil},
		{"empty EventBook", &pb.EventBook{}, nil},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := CoverOf(tt.input)
			if got != tt.want {
				t.Errorf("got %v, want %v", got, tt.want)
			}
		})
	}
}

func TestDomain(t *testing.T) {
	tests := []struct {
		name  string
		input interface{}
		want  string
	}{
		{"with domain", &pb.EventBook{Cover: &pb.Cover{Domain: "orders"}}, "orders"},
		{"empty domain", &pb.EventBook{Cover: &pb.Cover{Domain: ""}}, UnknownDomain},
		{"nil cover", &pb.EventBook{}, UnknownDomain},
		{"nil input", nil, UnknownDomain},
		{"unknown type", 42, UnknownDomain},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := Domain(tt.input)
			if got != tt.want {
				t.Errorf("got %q, want %q", got, tt.want)
			}
		})
	}
}

func TestCorrelationID(t *testing.T) {
	tests := []struct {
		name  string
		input interface{}
		want  string
	}{
		{"with correlation ID", &pb.Cover{CorrelationId: "corr-123"}, "corr-123"},
		{"empty correlation ID", &pb.Cover{}, ""},
		{"nil cover", nil, ""},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := CorrelationID(tt.input)
			if got != tt.want {
				t.Errorf("got %q, want %q", got, tt.want)
			}
		})
	}
}

func TestHasCorrelationID(t *testing.T) {
	tests := []struct {
		name  string
		input interface{}
		want  bool
	}{
		{"with correlation ID", &pb.Cover{CorrelationId: "corr-123"}, true},
		{"empty correlation ID", &pb.Cover{}, false},
		{"nil cover", nil, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := HasCorrelationID(tt.input)
			if got != tt.want {
				t.Errorf("got %v, want %v", got, tt.want)
			}
		})
	}
}

func TestRootUUID(t *testing.T) {
	validID := uuid.New()

	t.Run("valid UUID", func(t *testing.T) {
		cover := &pb.Cover{Root: UUIDToProto(validID)}
		got, ok := RootUUID(cover)
		if !ok {
			t.Fatal("expected ok to be true")
		}
		if got != validID {
			t.Errorf("got %v, want %v", got, validID)
		}
	})

	t.Run("nil root", func(t *testing.T) {
		cover := &pb.Cover{}
		_, ok := RootUUID(cover)
		if ok {
			t.Error("expected ok to be false for nil root")
		}
	})

	t.Run("nil cover", func(t *testing.T) {
		_, ok := RootUUID(nil)
		if ok {
			t.Error("expected ok to be false for nil input")
		}
	})

	t.Run("invalid UUID bytes", func(t *testing.T) {
		cover := &pb.Cover{Root: &pb.UUID{Value: []byte{1, 2, 3}}}
		_, ok := RootUUID(cover)
		if ok {
			t.Error("expected ok to be false for invalid bytes")
		}
	})
}

func TestRootIDHex(t *testing.T) {
	validID := uuid.MustParse("12345678-1234-1234-1234-123456789abc")

	t.Run("valid UUID", func(t *testing.T) {
		cover := &pb.Cover{Root: UUIDToProto(validID)}
		got := RootIDHex(cover)
		// UUID bytes are in standard form, hex encoded
		if got == "" {
			t.Error("expected non-empty hex string")
		}
	})

	t.Run("nil root", func(t *testing.T) {
		cover := &pb.Cover{}
		got := RootIDHex(cover)
		if got != "" {
			t.Errorf("expected empty string, got %q", got)
		}
	})

	t.Run("nil cover", func(t *testing.T) {
		got := RootIDHex(nil)
		if got != "" {
			t.Errorf("expected empty string, got %q", got)
		}
	})
}

func TestEdition(t *testing.T) {
	tests := []struct {
		name  string
		input interface{}
		want  string
	}{
		{"with edition", &pb.Cover{Edition: &pb.Edition{Name: "test-edition"}}, "test-edition"},
		{"empty edition name", &pb.Cover{Edition: &pb.Edition{Name: ""}}, DefaultEdition},
		{"nil edition", &pb.Cover{}, DefaultEdition},
		{"nil cover", nil, DefaultEdition},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := Edition(tt.input)
			if got != tt.want {
				t.Errorf("got %q, want %q", got, tt.want)
			}
		})
	}
}

func TestEditionOpt(t *testing.T) {
	t.Run("with edition", func(t *testing.T) {
		cover := &pb.Cover{Edition: &pb.Edition{Name: "test-edition"}}
		got := EditionOpt(cover)
		if got == nil {
			t.Fatal("expected non-nil result")
		}
		if *got != "test-edition" {
			t.Errorf("got %q, want %q", *got, "test-edition")
		}
	})

	t.Run("empty edition name", func(t *testing.T) {
		cover := &pb.Cover{Edition: &pb.Edition{Name: ""}}
		got := EditionOpt(cover)
		if got != nil {
			t.Error("expected nil for empty edition name")
		}
	})

	t.Run("nil edition", func(t *testing.T) {
		cover := &pb.Cover{}
		got := EditionOpt(cover)
		if got != nil {
			t.Error("expected nil for nil edition")
		}
	})

	t.Run("nil cover", func(t *testing.T) {
		got := EditionOpt(nil)
		if got != nil {
			t.Error("expected nil for nil input")
		}
	})
}

func TestRoutingKey(t *testing.T) {
	cover := &pb.Cover{Domain: "orders"}
	got := RoutingKey(cover)
	if got != "orders" {
		t.Errorf("got %q, want %q", got, "orders")
	}
}

func TestCacheKey(t *testing.T) {
	id := uuid.New()
	cover := &pb.Cover{Domain: "orders", Root: UUIDToProto(id)}
	got := CacheKey(cover)
	if got == "" {
		t.Error("expected non-empty cache key")
	}
	// Should contain domain
	if len(got) < 7 {
		t.Error("cache key too short")
	}
}

func TestUUIDToProto(t *testing.T) {
	id := uuid.New()
	proto := UUIDToProto(id)

	if proto == nil {
		t.Fatal("expected non-nil proto")
	}
	if len(proto.Value) != 16 {
		t.Errorf("expected 16 bytes, got %d", len(proto.Value))
	}
}

func TestProtoToUUID(t *testing.T) {
	t.Run("valid UUID", func(t *testing.T) {
		original := uuid.New()
		proto := UUIDToProto(original)
		got, err := ProtoToUUID(proto)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if got != original {
			t.Errorf("got %v, want %v", got, original)
		}
	})

	t.Run("nil proto", func(t *testing.T) {
		_, err := ProtoToUUID(nil)
		if err == nil {
			t.Error("expected error for nil input")
		}
	})

	t.Run("invalid bytes", func(t *testing.T) {
		proto := &pb.UUID{Value: []byte{1, 2, 3}}
		_, err := ProtoToUUID(proto)
		if err == nil {
			t.Error("expected error for invalid bytes")
		}
	})
}

func TestMainTimeline(t *testing.T) {
	edition := MainTimeline()
	if edition == nil {
		t.Fatal("expected non-nil edition")
	}
	if edition.Name != DefaultEdition {
		t.Errorf("got %q, want %q", edition.Name, DefaultEdition)
	}
}

func TestImplicitEdition(t *testing.T) {
	edition := ImplicitEdition("my-edition")
	if edition == nil {
		t.Fatal("expected non-nil edition")
	}
	if edition.Name != "my-edition" {
		t.Errorf("got %q, want %q", edition.Name, "my-edition")
	}
	if len(edition.Divergences) != 0 {
		t.Error("expected no divergences")
	}
}

func TestExplicitEdition(t *testing.T) {
	divergences := []*pb.DomainDivergence{
		{Domain: "orders", Sequence: 10},
		{Domain: "inventory", Sequence: 5},
	}
	edition := ExplicitEdition("branch", divergences)

	if edition == nil {
		t.Fatal("expected non-nil edition")
	}
	if edition.Name != "branch" {
		t.Errorf("got %q, want %q", edition.Name, "branch")
	}
	if len(edition.Divergences) != 2 {
		t.Errorf("expected 2 divergences, got %d", len(edition.Divergences))
	}
}

func TestIsMainTimeline(t *testing.T) {
	tests := []struct {
		name    string
		edition *pb.Edition
		want    bool
	}{
		{"nil edition", nil, true},
		{"empty name", &pb.Edition{Name: ""}, true},
		{"default edition", &pb.Edition{Name: DefaultEdition}, true},
		{"custom edition", &pb.Edition{Name: "custom"}, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := IsMainTimeline(tt.edition)
			if got != tt.want {
				t.Errorf("got %v, want %v", got, tt.want)
			}
		})
	}
}

func TestDivergenceFor(t *testing.T) {
	edition := &pb.Edition{
		Name: "branch",
		Divergences: []*pb.DomainDivergence{
			{Domain: "orders", Sequence: 10},
			{Domain: "inventory", Sequence: 5},
		},
	}

	tests := []struct {
		name    string
		edition *pb.Edition
		domain  string
		want    int64
	}{
		{"existing domain", edition, "orders", 10},
		{"another domain", edition, "inventory", 5},
		{"missing domain", edition, "shipping", -1},
		{"nil edition", nil, "orders", -1},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := DivergenceFor(tt.edition, tt.domain)
			if got != tt.want {
				t.Errorf("got %d, want %d", got, tt.want)
			}
		})
	}
}

func TestNextSequence(t *testing.T) {
	tests := []struct {
		name string
		book *pb.EventBook
		want uint32
	}{
		{"with next sequence", &pb.EventBook{NextSequence: 42}, 42},
		{"zero sequence", &pb.EventBook{NextSequence: 0}, 0},
		{"nil book", nil, 0},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := NextSequence(tt.book)
			if got != tt.want {
				t.Errorf("got %d, want %d", got, tt.want)
			}
		})
	}
}

func TestEventPages(t *testing.T) {
	pages := []*pb.EventPage{{}, {}}

	t.Run("with pages", func(t *testing.T) {
		book := &pb.EventBook{Pages: pages}
		got := EventPages(book)
		if len(got) != 2 {
			t.Errorf("expected 2 pages, got %d", len(got))
		}
	})

	t.Run("nil book", func(t *testing.T) {
		got := EventPages(nil)
		if got != nil {
			t.Error("expected nil for nil book")
		}
	})

	t.Run("empty pages", func(t *testing.T) {
		book := &pb.EventBook{}
		got := EventPages(book)
		if len(got) != 0 {
			t.Error("expected empty slice")
		}
	})
}

func TestCommandPages(t *testing.T) {
	pages := []*pb.CommandPage{{}, {}}

	t.Run("with pages", func(t *testing.T) {
		book := &pb.CommandBook{Pages: pages}
		got := CommandPages(book)
		if len(got) != 2 {
			t.Errorf("expected 2 pages, got %d", len(got))
		}
	})

	t.Run("nil book", func(t *testing.T) {
		got := CommandPages(nil)
		if got != nil {
			t.Error("expected nil for nil book")
		}
	})
}

func TestEventsFromResponse(t *testing.T) {
	pages := []*pb.EventPage{{}, {}, {}}

	t.Run("with events", func(t *testing.T) {
		resp := &pb.CommandResponse{Events: &pb.EventBook{Pages: pages}}
		got := EventsFromResponse(resp)
		if len(got) != 3 {
			t.Errorf("expected 3 pages, got %d", len(got))
		}
	})

	t.Run("nil response", func(t *testing.T) {
		got := EventsFromResponse(nil)
		if got != nil {
			t.Error("expected nil for nil response")
		}
	})

	t.Run("nil events", func(t *testing.T) {
		resp := &pb.CommandResponse{}
		got := EventsFromResponse(resp)
		if got != nil {
			t.Error("expected nil for nil events")
		}
	})
}

func TestTypeURL(t *testing.T) {
	got := TypeURL("examples", "CreateCart")
	want := "type.googleapis.com/examples.CreateCart"
	if got != want {
		t.Errorf("got %q, want %q", got, want)
	}
}

func TestTypeNameFromURL(t *testing.T) {
	tests := []struct {
		name    string
		typeURL string
		want    string
	}{
		// Standard case: package.TypeName after the /
		{"full type URL with dot", "type.googleapis.com/examples.CreateCart", "CreateCart"},
		{"just type name", "CreateCart", "CreateCart"},
		// Note: current implementation splits by . first, then /
		// For URLs without a dot after the /, returns portion after last dot
		{"URL with slash only no package", "type.googleapis.com/CreateCart", "com/CreateCart"},
		{"empty string", "", ""},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := TypeNameFromURL(tt.typeURL)
			if got != tt.want {
				t.Errorf("got %q, want %q", got, tt.want)
			}
		})
	}
}

func TestTypeURLMatches(t *testing.T) {
	tests := []struct {
		name    string
		typeURL string
		suffix  string
		want    bool
	}{
		{"matches", "type.googleapis.com/examples.CreateCart", "CreateCart", true},
		{"does not match", "type.googleapis.com/examples.CreateCart", "RemoveItem", false},
		{"exact match", "CreateCart", "CreateCart", true},
		{"empty suffix", "type.googleapis.com/examples.CreateCart", "", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := TypeURLMatches(tt.typeURL, tt.suffix)
			if got != tt.want {
				t.Errorf("got %v, want %v", got, tt.want)
			}
		})
	}
}

func TestNow(t *testing.T) {
	before := time.Now()
	ts := Now()
	after := time.Now()

	if ts == nil {
		t.Fatal("expected non-nil timestamp")
	}

	tsTime := ts.AsTime()
	if tsTime.Before(before) || tsTime.After(after) {
		t.Error("timestamp not within expected range")
	}
}

func TestParseTimestamp(t *testing.T) {
	t.Run("valid RFC3339", func(t *testing.T) {
		ts, err := ParseTimestamp("2024-01-15T10:30:00Z")
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if ts == nil {
			t.Fatal("expected non-nil timestamp")
		}
		// Check the time is correct
		expected := time.Date(2024, 1, 15, 10, 30, 0, 0, time.UTC)
		if !ts.AsTime().Equal(expected) {
			t.Errorf("got %v, want %v", ts.AsTime(), expected)
		}
	})

	t.Run("with nanoseconds", func(t *testing.T) {
		ts, err := ParseTimestamp("2024-01-15T10:30:00.123456789Z")
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if ts.GetNanos() == 0 {
			t.Error("expected non-zero nanos")
		}
	})

	t.Run("invalid format", func(t *testing.T) {
		_, err := ParseTimestamp("not a timestamp")
		if err == nil {
			t.Error("expected error for invalid timestamp")
		}
		// Should be an InvalidTimestampError
		clientErr := AsClientError(err)
		if clientErr == nil {
			t.Error("expected ClientError")
		} else if clientErr.Kind != ErrInvalidTimestamp {
			t.Errorf("expected ErrInvalidTimestamp, got %v", clientErr.Kind)
		}
	})
}

func TestDecodeEvent(t *testing.T) {
	t.Run("nil page", func(t *testing.T) {
		var msg mockUnmarshaler
		got := DecodeEvent(nil, "Test", &msg)
		if got {
			t.Error("expected false for nil page")
		}
	})

	t.Run("nil event", func(t *testing.T) {
		page := &pb.EventPage{}
		var msg mockUnmarshaler
		got := DecodeEvent(page, "Test", &msg)
		if got {
			t.Error("expected false for nil event")
		}
	})

	t.Run("type mismatch", func(t *testing.T) {
		page := &pb.EventPage{
			Payload: &pb.EventPage_Event{Event: &anypb.Any{TypeUrl: "type.googleapis.com/examples.Other"}},
		}
		var msg mockUnmarshaler
		got := DecodeEvent(page, "CreateCart", &msg)
		if got {
			t.Error("expected false for type mismatch")
		}
	})

	t.Run("successful decode", func(t *testing.T) {
		page := &pb.EventPage{
			Payload: &pb.EventPage_Event{Event: &anypb.Any{TypeUrl: "type.googleapis.com/examples.CreateCart", Value: []byte{}}},
		}
		msg := &mockUnmarshaler{shouldSucceed: true}
		got := DecodeEvent(page, "CreateCart", msg)
		if !got {
			t.Error("expected true for successful decode")
		}
	})

	t.Run("unmarshal failure", func(t *testing.T) {
		page := &pb.EventPage{
			Payload: &pb.EventPage_Event{Event: &anypb.Any{TypeUrl: "type.googleapis.com/examples.CreateCart", Value: []byte{}}},
		}
		msg := &mockUnmarshaler{shouldSucceed: false}
		got := DecodeEvent(page, "CreateCart", msg)
		if got {
			t.Error("expected false for unmarshal failure")
		}
	})
}

type mockUnmarshaler struct {
	shouldSucceed bool
}

func (m *mockUnmarshaler) Unmarshal(data []byte) error {
	if m.shouldSucceed {
		return nil
	}
	return InvalidArgumentError("unmarshal failed")
}

func TestNewCover(t *testing.T) {
	id := uuid.New()
	cover := NewCover("orders", id, "corr-123")

	if cover.Domain != "orders" {
		t.Errorf("got domain %q, want %q", cover.Domain, "orders")
	}
	if cover.CorrelationId != "corr-123" {
		t.Errorf("got correlation_id %q, want %q", cover.CorrelationId, "corr-123")
	}
	if cover.Root == nil {
		t.Error("expected non-nil root")
	}
}

func TestNewCoverWithEdition(t *testing.T) {
	id := uuid.New()
	edition := ImplicitEdition("test-edition")
	cover := NewCoverWithEdition("orders", id, "corr-123", edition)

	if cover.Domain != "orders" {
		t.Errorf("got domain %q, want %q", cover.Domain, "orders")
	}
	if cover.Edition != edition {
		t.Error("edition mismatch")
	}
}

func TestNewCommandPage(t *testing.T) {
	page := NewCommandPage(5, nil)
	if page.Sequence != 5 {
		t.Errorf("got sequence %d, want %d", page.Sequence, 5)
	}
}

func TestNewCommandBook(t *testing.T) {
	cover := &pb.Cover{Domain: "test"}
	pages := []*pb.CommandPage{{Sequence: 1}, {Sequence: 2}}
	book := NewCommandBook(cover, pages...)

	if book.Cover != cover {
		t.Error("cover mismatch")
	}
	if len(book.Pages) != 2 {
		t.Errorf("expected 2 pages, got %d", len(book.Pages))
	}
}

func TestNewQueryWithRange(t *testing.T) {
	cover := &pb.Cover{Domain: "test"}

	t.Run("without upper bound", func(t *testing.T) {
		query := NewQueryWithRange(cover, 5, nil)
		if query.Cover != cover {
			t.Error("cover mismatch")
		}
		rangeSelect := query.GetRange()
		if rangeSelect == nil {
			t.Fatal("expected range selection")
		}
		if rangeSelect.Lower != 5 {
			t.Errorf("got lower %d, want %d", rangeSelect.Lower, 5)
		}
	})

	t.Run("with upper bound", func(t *testing.T) {
		upper := uint32(10)
		query := NewQueryWithRange(cover, 5, &upper)
		rangeSelect := query.GetRange()
		if rangeSelect == nil {
			t.Fatal("expected range selection")
		}
		if *rangeSelect.Upper != 10 {
			t.Errorf("got upper %d, want %d", *rangeSelect.Upper, 10)
		}
	})
}

func TestNewQueryWithTemporal(t *testing.T) {
	cover := &pb.Cover{Domain: "test"}
	temporal := &pb.TemporalQuery{
		PointInTime: &pb.TemporalQuery_AsOfSequence{AsOfSequence: 42},
	}
	query := NewQueryWithTemporal(cover, temporal)

	if query.Cover != cover {
		t.Error("cover mismatch")
	}
	if query.GetTemporal() != temporal {
		t.Error("temporal mismatch")
	}
}

func TestRangeSelection(t *testing.T) {
	t.Run("without upper", func(t *testing.T) {
		sel := RangeSelection(5, nil)
		if sel.Range.Lower != 5 {
			t.Errorf("got lower %d, want %d", sel.Range.Lower, 5)
		}
		if sel.Range.Upper != nil {
			t.Error("expected nil upper")
		}
	})

	t.Run("with upper", func(t *testing.T) {
		upper := uint32(10)
		sel := RangeSelection(5, &upper)
		if sel.Range.Lower != 5 {
			t.Errorf("got lower %d, want %d", sel.Range.Lower, 5)
		}
		if *sel.Range.Upper != 10 {
			t.Errorf("got upper %d, want %d", *sel.Range.Upper, 10)
		}
	})
}

func TestTemporalSelectionBySequence(t *testing.T) {
	sel := TemporalSelectionBySequence(42)
	if sel.Temporal.GetAsOfSequence() != 42 {
		t.Errorf("got %d, want %d", sel.Temporal.GetAsOfSequence(), 42)
	}
}

func TestTemporalSelectionByTime(t *testing.T) {
	ts := timestamppb.Now()
	sel := TemporalSelectionByTime(ts)
	if sel.Temporal.GetAsOfTime() != ts {
		t.Error("timestamp mismatch")
	}
}
