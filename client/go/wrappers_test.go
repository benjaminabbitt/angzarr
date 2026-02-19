package angzarr

import (
	"testing"

	"github.com/google/uuid"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/types/known/anypb"
)

func TestEventBookW_Constructor(t *testing.T) {
	proto := &pb.EventBook{NextSequence: 5}
	wrapper := NewEventBookW(proto)

	if wrapper.EventBook != proto {
		t.Error("expected embedded proto to match")
	}
}

func TestEventBookW_NextSequence(t *testing.T) {
	tests := []struct {
		name string
		seq  uint32
		want uint32
	}{
		{"returns value", 5, 5},
		{"returns zero", 0, 0},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			wrapper := NewEventBookW(&pb.EventBook{NextSequence: tt.seq})
			if got := wrapper.NextSequence(); got != tt.want {
				t.Errorf("got %d, want %d", got, tt.want)
			}
		})
	}
}

func TestEventBookW_Pages(t *testing.T) {
	t.Run("returns wrapped pages", func(t *testing.T) {
		proto := &pb.EventBook{Pages: []*pb.EventPage{{}, {}}}
		wrapper := NewEventBookW(proto)
		pages := wrapper.Pages()
		if len(pages) != 2 {
			t.Errorf("expected 2 pages, got %d", len(pages))
		}
		// Verify they are wrapped EventPageW instances
		if pages[0].EventPage == nil {
			t.Error("expected wrapped EventPageW")
		}
	})

	t.Run("returns empty slice", func(t *testing.T) {
		wrapper := NewEventBookW(&pb.EventBook{})
		if len(wrapper.Pages()) != 0 {
			t.Error("expected empty slice")
		}
	})
}

func TestEventBookW_Domain(t *testing.T) {
	tests := []struct {
		name  string
		cover *pb.Cover
		want  string
	}{
		{"with domain", &pb.Cover{Domain: "orders"}, "orders"},
		{"empty domain", &pb.Cover{Domain: ""}, UnknownDomain},
		{"nil cover", nil, UnknownDomain},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			wrapper := NewEventBookW(&pb.EventBook{Cover: tt.cover})
			if got := wrapper.Domain(); got != tt.want {
				t.Errorf("got %q, want %q", got, tt.want)
			}
		})
	}
}

func TestEventBookW_CorrelationID(t *testing.T) {
	tests := []struct {
		name  string
		cover *pb.Cover
		want  string
	}{
		{"with correlation ID", &pb.Cover{CorrelationId: "corr-123"}, "corr-123"},
		{"empty correlation ID", &pb.Cover{}, ""},
		{"nil cover", nil, ""},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			wrapper := NewEventBookW(&pb.EventBook{Cover: tt.cover})
			if got := wrapper.CorrelationID(); got != tt.want {
				t.Errorf("got %q, want %q", got, tt.want)
			}
		})
	}
}

func TestEventBookW_HasCorrelationID(t *testing.T) {
	tests := []struct {
		name  string
		cover *pb.Cover
		want  bool
	}{
		{"with correlation ID", &pb.Cover{CorrelationId: "xyz"}, true},
		{"empty correlation ID", &pb.Cover{}, false},
		{"nil cover", nil, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			wrapper := NewEventBookW(&pb.EventBook{Cover: tt.cover})
			if got := wrapper.HasCorrelationID(); got != tt.want {
				t.Errorf("got %v, want %v", got, tt.want)
			}
		})
	}
}

func TestEventBookW_RootUUID(t *testing.T) {
	t.Run("valid UUID", func(t *testing.T) {
		id := uuid.New()
		wrapper := NewEventBookW(&pb.EventBook{
			Cover: &pb.Cover{Root: UUIDToProto(id)},
		})
		got, ok := wrapper.RootUUID()
		if !ok {
			t.Fatal("expected ok to be true")
		}
		if got != id {
			t.Errorf("got %v, want %v", got, id)
		}
	})

	t.Run("nil root", func(t *testing.T) {
		wrapper := NewEventBookW(&pb.EventBook{Cover: &pb.Cover{}})
		_, ok := wrapper.RootUUID()
		if ok {
			t.Error("expected ok to be false")
		}
	})

	t.Run("nil cover", func(t *testing.T) {
		wrapper := NewEventBookW(&pb.EventBook{})
		_, ok := wrapper.RootUUID()
		if ok {
			t.Error("expected ok to be false")
		}
	})
}

func TestEventBookW_Edition(t *testing.T) {
	tests := []struct {
		name  string
		cover *pb.Cover
		want  string
	}{
		{"with edition", &pb.Cover{Edition: &pb.Edition{Name: "v2"}}, "v2"},
		{"empty edition", &pb.Cover{Edition: &pb.Edition{Name: ""}}, DefaultEdition},
		{"nil edition", &pb.Cover{}, DefaultEdition},
		{"nil cover", nil, DefaultEdition},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			wrapper := NewEventBookW(&pb.EventBook{Cover: tt.cover})
			if got := wrapper.Edition(); got != tt.want {
				t.Errorf("got %q, want %q", got, tt.want)
			}
		})
	}
}

func TestEventBookW_RoutingKey(t *testing.T) {
	wrapper := NewEventBookW(&pb.EventBook{Cover: &pb.Cover{Domain: "inventory"}})
	if got := wrapper.RoutingKey(); got != "inventory" {
		t.Errorf("got %q, want %q", got, "inventory")
	}
}

func TestEventBookW_CacheKey(t *testing.T) {
	id := uuid.New()
	wrapper := NewEventBookW(&pb.EventBook{
		Cover: &pb.Cover{Domain: "orders", Root: UUIDToProto(id)},
	})
	got := wrapper.CacheKey()
	if got == "" {
		t.Error("expected non-empty cache key")
	}
}

func TestEventBookW_CoverWrapper(t *testing.T) {
	wrapper := NewEventBookW(&pb.EventBook{Cover: &pb.Cover{Domain: "test"}})
	coverW := wrapper.CoverWrapper()
	if coverW.Domain() != "test" {
		t.Errorf("got %q, want %q", coverW.Domain(), "test")
	}
}

func TestCommandBookW_Constructor(t *testing.T) {
	proto := &pb.CommandBook{}
	wrapper := NewCommandBookW(proto)
	if wrapper.CommandBook != proto {
		t.Error("expected embedded proto to match")
	}
}

func TestCommandBookW_Pages(t *testing.T) {
	t.Run("returns wrapped pages", func(t *testing.T) {
		proto := &pb.CommandBook{Pages: []*pb.CommandPage{{Sequence: 1}, {Sequence: 2}}}
		wrapper := NewCommandBookW(proto)
		pages := wrapper.Pages()
		if len(pages) != 2 {
			t.Errorf("expected 2 pages, got %d", len(pages))
		}
		// Verify they are wrapped CommandPageW instances
		if pages[0].Sequence() != 1 {
			t.Errorf("expected sequence 1, got %d", pages[0].Sequence())
		}
	})

	t.Run("returns empty slice", func(t *testing.T) {
		wrapper := NewCommandBookW(&pb.CommandBook{})
		if len(wrapper.Pages()) != 0 {
			t.Error("expected empty slice")
		}
	})
}

func TestCommandBookW_Domain(t *testing.T) {
	wrapper := NewCommandBookW(&pb.CommandBook{Cover: &pb.Cover{Domain: "fulfillment"}})
	if got := wrapper.Domain(); got != "fulfillment" {
		t.Errorf("got %q, want %q", got, "fulfillment")
	}
}

func TestCommandBookW_CorrelationID(t *testing.T) {
	wrapper := NewCommandBookW(&pb.CommandBook{Cover: &pb.Cover{CorrelationId: "cmd-456"}})
	if got := wrapper.CorrelationID(); got != "cmd-456" {
		t.Errorf("got %q, want %q", got, "cmd-456")
	}
}

func TestCoverW_Constructor(t *testing.T) {
	proto := &pb.Cover{Domain: "test"}
	wrapper := NewCoverW(proto)
	if wrapper.Cover != proto {
		t.Error("expected embedded proto to match")
	}
}

func TestCoverW_Domain(t *testing.T) {
	tests := []struct {
		name   string
		domain string
		want   string
	}{
		{"with domain", "orders", "orders"},
		{"empty domain", "", UnknownDomain},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			wrapper := NewCoverW(&pb.Cover{Domain: tt.domain})
			if got := wrapper.Domain(); got != tt.want {
				t.Errorf("got %q, want %q", got, tt.want)
			}
		})
	}
}

func TestCoverW_CorrelationID(t *testing.T) {
	tests := []struct {
		name string
		corr string
		want string
	}{
		{"with correlation ID", "abc-123", "abc-123"},
		{"empty correlation ID", "", ""},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			wrapper := NewCoverW(&pb.Cover{CorrelationId: tt.corr})
			if got := wrapper.CorrelationID(); got != tt.want {
				t.Errorf("got %q, want %q", got, tt.want)
			}
		})
	}
}

func TestCoverW_HasCorrelationID(t *testing.T) {
	tests := []struct {
		name string
		corr string
		want bool
	}{
		{"with correlation ID", "xyz", true},
		{"empty correlation ID", "", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			wrapper := NewCoverW(&pb.Cover{CorrelationId: tt.corr})
			if got := wrapper.HasCorrelationID(); got != tt.want {
				t.Errorf("got %v, want %v", got, tt.want)
			}
		})
	}
}

func TestCoverW_RootUUID(t *testing.T) {
	t.Run("valid UUID", func(t *testing.T) {
		id := uuid.New()
		wrapper := NewCoverW(&pb.Cover{Root: UUIDToProto(id)})
		got, ok := wrapper.RootUUID()
		if !ok {
			t.Fatal("expected ok to be true")
		}
		if got != id {
			t.Errorf("got %v, want %v", got, id)
		}
	})

	t.Run("nil root", func(t *testing.T) {
		wrapper := NewCoverW(&pb.Cover{})
		_, ok := wrapper.RootUUID()
		if ok {
			t.Error("expected ok to be false")
		}
	})

	t.Run("invalid bytes", func(t *testing.T) {
		wrapper := NewCoverW(&pb.Cover{Root: &pb.UUID{Value: []byte{1, 2, 3}}})
		_, ok := wrapper.RootUUID()
		if ok {
			t.Error("expected ok to be false")
		}
	})
}

func TestCoverW_RootIDHex(t *testing.T) {
	t.Run("valid UUID", func(t *testing.T) {
		id := uuid.New()
		wrapper := NewCoverW(&pb.Cover{Root: UUIDToProto(id)})
		got := wrapper.RootIDHex()
		if got == "" {
			t.Error("expected non-empty hex string")
		}
	})

	t.Run("nil root", func(t *testing.T) {
		wrapper := NewCoverW(&pb.Cover{})
		if got := wrapper.RootIDHex(); got != "" {
			t.Errorf("expected empty string, got %q", got)
		}
	})
}

func TestCoverW_Edition(t *testing.T) {
	tests := []struct {
		name    string
		edition *pb.Edition
		want    string
	}{
		{"with edition", &pb.Edition{Name: "speculative"}, "speculative"},
		{"empty edition", &pb.Edition{Name: ""}, DefaultEdition},
		{"nil edition", nil, DefaultEdition},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			wrapper := NewCoverW(&pb.Cover{Edition: tt.edition})
			if got := wrapper.Edition(); got != tt.want {
				t.Errorf("got %q, want %q", got, tt.want)
			}
		})
	}
}

func TestCoverW_EditionOpt(t *testing.T) {
	t.Run("with edition", func(t *testing.T) {
		wrapper := NewCoverW(&pb.Cover{Edition: &pb.Edition{Name: "branch-a"}})
		got := wrapper.EditionOpt()
		if got == nil || *got != "branch-a" {
			t.Errorf("expected 'branch-a', got %v", got)
		}
	})

	t.Run("nil edition", func(t *testing.T) {
		wrapper := NewCoverW(&pb.Cover{})
		if got := wrapper.EditionOpt(); got != nil {
			t.Error("expected nil")
		}
	})
}

func TestCoverW_RoutingKey(t *testing.T) {
	wrapper := NewCoverW(&pb.Cover{Domain: "payments"})
	if got := wrapper.RoutingKey(); got != "payments" {
		t.Errorf("got %q, want %q", got, "payments")
	}
}

func TestCoverW_CacheKey(t *testing.T) {
	id := uuid.New()
	wrapper := NewCoverW(&pb.Cover{Domain: "inventory", Root: UUIDToProto(id)})
	got := wrapper.CacheKey()
	if got == "" {
		t.Error("expected non-empty cache key")
	}
}

func TestQueryW_Constructor(t *testing.T) {
	proto := &pb.Query{}
	wrapper := NewQueryW(proto)
	if wrapper.Query != proto {
		t.Error("expected embedded proto to match")
	}
}

func TestQueryW_Domain(t *testing.T) {
	wrapper := NewQueryW(&pb.Query{Cover: &pb.Cover{Domain: "shipping"}})
	if got := wrapper.Domain(); got != "shipping" {
		t.Errorf("got %q, want %q", got, "shipping")
	}
}

func TestQueryW_CorrelationID(t *testing.T) {
	wrapper := NewQueryW(&pb.Query{Cover: &pb.Cover{CorrelationId: "query-789"}})
	if got := wrapper.CorrelationID(); got != "query-789" {
		t.Errorf("got %q, want %q", got, "query-789")
	}
}

func TestEventPageW_Constructor(t *testing.T) {
	proto := &pb.EventPage{}
	wrapper := NewEventPageW(proto)
	if wrapper.EventPage != proto {
		t.Error("expected embedded proto to match")
	}
}

func TestEventPageW_DecodeEvent(t *testing.T) {
	t.Run("successful decode", func(t *testing.T) {
		page := &pb.EventPage{
			Payload: &pb.EventPage_Event{Event: &anypb.Any{TypeUrl: "type.googleapis.com/examples.CreateCart", Value: []byte{}}},
		}
		wrapper := NewEventPageW(page)
		msg := &mockUnmarshaler{shouldSucceed: true}
		if !wrapper.DecodeEvent("CreateCart", msg) {
			t.Error("expected true for successful decode")
		}
	})

	t.Run("type mismatch", func(t *testing.T) {
		page := &pb.EventPage{
			Payload: &pb.EventPage_Event{Event: &anypb.Any{TypeUrl: "type.googleapis.com/examples.Other"}},
		}
		wrapper := NewEventPageW(page)
		msg := &mockUnmarshaler{shouldSucceed: true}
		if wrapper.DecodeEvent("CreateCart", msg) {
			t.Error("expected false for type mismatch")
		}
	})

	t.Run("nil event", func(t *testing.T) {
		wrapper := NewEventPageW(&pb.EventPage{})
		msg := &mockUnmarshaler{shouldSucceed: true}
		if wrapper.DecodeEvent("Test", msg) {
			t.Error("expected false for nil event")
		}
	})
}

func TestCommandPageW_Constructor(t *testing.T) {
	proto := &pb.CommandPage{Sequence: 10}
	wrapper := NewCommandPageW(proto)
	if wrapper.CommandPage != proto {
		t.Error("expected embedded proto to match")
	}
}

func TestCommandPageW_Sequence(t *testing.T) {
	wrapper := NewCommandPageW(&pb.CommandPage{Sequence: 42})
	if got := wrapper.Sequence(); got != 42 {
		t.Errorf("got %d, want %d", got, 42)
	}
}

func TestCommandResponseW_Constructor(t *testing.T) {
	proto := &pb.CommandResponse{}
	wrapper := NewCommandResponseW(proto)
	if wrapper.CommandResponse != proto {
		t.Error("expected embedded proto to match")
	}
}

func TestCommandResponseW_EventsBook(t *testing.T) {
	t.Run("returns wrapped EventBookW", func(t *testing.T) {
		proto := &pb.CommandResponse{
			Events: &pb.EventBook{NextSequence: 5, Pages: []*pb.EventPage{{}}},
		}
		wrapper := NewCommandResponseW(proto)
		book := wrapper.EventsBook()
		if book == nil {
			t.Fatal("expected non-nil EventBookW")
		}
		if book.NextSequence() != 5 {
			t.Errorf("expected next_sequence 5, got %d", book.NextSequence())
		}
	})

	t.Run("returns nil when not set", func(t *testing.T) {
		wrapper := NewCommandResponseW(&pb.CommandResponse{})
		if wrapper.EventsBook() != nil {
			t.Error("expected nil")
		}
	})
}

func TestCommandResponseW_Events(t *testing.T) {
	t.Run("returns wrapped events", func(t *testing.T) {
		proto := &pb.CommandResponse{
			Events: &pb.EventBook{Pages: []*pb.EventPage{{}, {}}},
		}
		wrapper := NewCommandResponseW(proto)
		events := wrapper.Events()
		if len(events) != 2 {
			t.Errorf("expected 2 pages, got %d", len(events))
		}
		// Verify they are wrapped EventPageW instances
		if events[0].EventPage == nil {
			t.Error("expected wrapped EventPageW")
		}
	})

	t.Run("nil events", func(t *testing.T) {
		wrapper := NewCommandResponseW(&pb.CommandResponse{})
		if len(wrapper.Events()) != 0 {
			t.Error("expected empty slice")
		}
	})
}
