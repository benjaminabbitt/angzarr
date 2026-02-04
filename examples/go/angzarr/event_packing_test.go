package angzarr

import (
	"testing"

	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/wrapperspb"

	angzarrpb "angzarr/proto/angzarr"
)

func TestPackEvent_singleEvent_returnsOnePageEventBook(t *testing.T) {
	cover := &angzarrpb.Cover{Root: &angzarrpb.UUID{Value: []byte("test-root")}}
	event := wrapperspb.String("hello")

	book, err := PackEvent(cover, event, 0)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if book.Cover != cover {
		t.Error("cover not preserved")
	}
	if len(book.Pages) != 1 {
		t.Fatalf("expected 1 page, got %d", len(book.Pages))
	}
	page := book.Pages[0]
	if seq, ok := page.Sequence.(*angzarrpb.EventPage_Num); !ok || seq.Num != 0 {
		t.Errorf("expected sequence 0, got %v", page.Sequence)
	}
	if page.Event == nil {
		t.Fatal("event is nil")
	}
	if page.CreatedAt == nil {
		t.Error("created_at is nil")
	}
}

func TestPackEvent_sequencePreserved(t *testing.T) {
	cover := &angzarrpb.Cover{Root: &angzarrpb.UUID{Value: []byte("root")}}
	book, err := PackEvent(cover, wrapperspb.String("x"), 42)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	seq := book.Pages[0].Sequence.(*angzarrpb.EventPage_Num)
	if seq.Num != 42 {
		t.Errorf("expected sequence 42, got %d", seq.Num)
	}
}

func TestPackEvents_multipleEvents_returnsSequentialPages(t *testing.T) {
	cover := &angzarrpb.Cover{Root: &angzarrpb.UUID{Value: []byte("root")}}
	msgs := []proto.Message{
		wrapperspb.String("first"),
		wrapperspb.String("second"),
		wrapperspb.String("third"),
	}

	book, err := PackEvents(cover, msgs, 5)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(book.Pages) != 3 {
		t.Fatalf("expected 3 pages, got %d", len(book.Pages))
	}
	for i, page := range book.Pages {
		seq := page.Sequence.(*angzarrpb.EventPage_Num)
		want := uint32(5 + i)
		if seq.Num != want {
			t.Errorf("page %d: expected sequence %d, got %d", i, want, seq.Num)
		}
		if page.Event == nil {
			t.Errorf("page %d: event is nil", i)
		}
		if page.CreatedAt == nil {
			t.Errorf("page %d: created_at is nil", i)
		}
	}
}

func TestPackEvents_emptySlice_returnsEmptyPages(t *testing.T) {
	cover := &angzarrpb.Cover{Root: &angzarrpb.UUID{Value: []byte("root")}}
	book, err := PackEvents(cover, nil, 0)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(book.Pages) != 0 {
		t.Errorf("expected 0 pages, got %d", len(book.Pages))
	}
}
