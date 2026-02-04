package angzarr

import (
	"context"
	"testing"

	angzarrpb "angzarr/proto/angzarr"
)

func TestProjectorHandler_GetDescriptor(t *testing.T) {
	h := NewProjectorHandler("web", "customer", "order", "product")

	resp, err := h.GetDescriptor(context.Background(), &angzarrpb.GetDescriptorRequest{})
	if err != nil {
		t.Fatal(err)
	}
	if resp.Name != "web" {
		t.Errorf("expected name 'web', got %q", resp.Name)
	}
	if resp.ComponentType != ComponentProjector {
		t.Errorf("expected type %q, got %q", ComponentProjector, resp.ComponentType)
	}
	if len(resp.Inputs) != 3 {
		t.Fatalf("expected 3 inputs, got %d", len(resp.Inputs))
	}
	domains := []string{"customer", "order", "product"}
	for i, want := range domains {
		if resp.Inputs[i].Domain != want {
			t.Errorf("input %d: expected domain %q, got %q", i, want, resp.Inputs[i].Domain)
		}
	}
}

func TestProjectorHandler_HandleDefault(t *testing.T) {
	h := NewProjectorHandler("test")

	resp, err := h.Handle(context.Background(), &angzarrpb.EventBook{})
	if err != nil {
		t.Fatal(err)
	}
	if resp == nil {
		t.Fatal("expected non-nil projection")
	}
}

func TestProjectorHandler_HandleCustom(t *testing.T) {
	h := NewProjectorHandler("test").
		WithHandle(func(book *angzarrpb.EventBook) (*angzarrpb.Projection, error) {
			return &angzarrpb.Projection{
				Projector: "test",
				Sequence:  uint32(len(book.Pages)),
			}, nil
		})

	resp, err := h.Handle(context.Background(), &angzarrpb.EventBook{
		Pages: []*angzarrpb.EventPage{
			{Sequence: &angzarrpb.EventPage_Num{Num: 0}},
			{Sequence: &angzarrpb.EventPage_Num{Num: 1}},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if resp.Projector != "test" {
		t.Errorf("expected projector 'test', got %q", resp.Projector)
	}
	if resp.Sequence != 2 {
		t.Errorf("expected sequence 2, got %d", resp.Sequence)
	}
}

func TestProjectorHandler_Descriptor(t *testing.T) {
	h := NewProjectorHandler("accounting", "order", "customer")

	desc := h.Descriptor()
	if desc.Name != "accounting" {
		t.Errorf("expected name 'accounting', got %q", desc.Name)
	}
	if desc.ComponentType != ComponentProjector {
		t.Errorf("expected type %q, got %q", ComponentProjector, desc.ComponentType)
	}
	if len(desc.Inputs) != 2 {
		t.Fatalf("expected 2 inputs, got %d", len(desc.Inputs))
	}
}
