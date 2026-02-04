package angzarr

import (
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"

	angzarrpb "angzarr/proto/angzarr"
)

// PackEvent wraps a single protobuf event into an EventBook.
func PackEvent(cover *angzarrpb.Cover, event proto.Message, seq uint32) (*angzarrpb.EventBook, error) {
	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return &angzarrpb.EventBook{
		Cover: cover,
		Pages: []*angzarrpb.EventPage{
			{
				Sequence:  &angzarrpb.EventPage_Num{Num: seq},
				Event:     eventAny,
				CreatedAt: timestamppb.Now(),
			},
		},
	}, nil
}

// PackEvents wraps multiple protobuf events into an EventBook with sequential numbering.
func PackEvents(cover *angzarrpb.Cover, events []proto.Message, startSeq uint32) (*angzarrpb.EventBook, error) {
	pages := make([]*angzarrpb.EventPage, 0, len(events))
	for i, event := range events {
		eventAny, err := anypb.New(event)
		if err != nil {
			return nil, err
		}
		pages = append(pages, &angzarrpb.EventPage{
			Sequence:  &angzarrpb.EventPage_Num{Num: startSeq + uint32(i)},
			Event:     eventAny,
			CreatedAt: timestamppb.Now(),
		})
	}

	return &angzarrpb.EventBook{
		Cover: cover,
		Pages: pages,
	}, nil
}
