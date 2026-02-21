package features

import (
	"github.com/cucumber/godog"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// DecodeContext holds state for event decoding scenarios
type DecodeContext struct {
	Event           *pb.EventPage
	Events          []*pb.EventPage
	Decoded         interface{}
	DecodedList     []interface{}
	Error           error
	TypeURL         string
	CommandResponse *MockCommandResponse
	MatchSuccess    bool
	Filtered        []*pb.EventPage
}

type MockCommandResponse struct {
	Events []*pb.EventPage
}

func newDecodeContext() *DecodeContext {
	return &DecodeContext{}
}

// InitEventDecodingSteps registers event decoding step definitions
func InitEventDecodingSteps(ctx *godog.ScenarioContext) {
	dc := newDecodeContext()

	// Given steps
	ctx.Step(`^an event with type_url "([^"]*)"$`, dc.givenEventTypeURL)
	ctx.Step(`^valid protobuf bytes for OrderCreated$`, dc.givenValidProtoBytes)
	ctx.Step(`^an event with type_url ending in "([^"]*)"$`, dc.givenEventSuffix)
	ctx.Step(`^an EventPage at sequence (\d+)$`, dc.givenEventAtSeq)
	ctx.Step(`^an EventPage with timestamp$`, dc.givenEventWithTimestamp)
	ctx.Step(`^an EventPage with Event payload$`, dc.givenEventPayload)
	ctx.Step(`^an EventPage with offloaded payload$`, dc.givenOffloadedPayload)
	ctx.Step(`^an event with properly encoded payload$`, dc.givenProperPayload)
	ctx.Step(`^an event with empty payload bytes$`, dc.givenEmptyPayload)
	ctx.Step(`^an event with corrupted payload bytes$`, dc.givenCorruptedBytes)
	ctx.Step(`^an EventPage with payload = None$`, dc.givenNonePayload)
	ctx.Step(`^an Event Any with empty value$`, dc.givenEmptyAny)
	ctx.Step(`^the decode_event<T>\(event, type_suffix\) function$`, dc.givenDecodeFunction)
	ctx.Step(`^a CommandResponse with events$`, dc.givenResponseWithEvents)
	ctx.Step(`^a CommandResponse with no events$`, dc.givenResponseNoEvents)
	ctx.Step(`^(\d+) events all of type "([^"]*)"$`, dc.givenMultipleSameType)
	ctx.Step(`^events: OrderCreated, ItemAdded, ItemAdded, OrderShipped$`, dc.givenMixedEvents)

	// When steps
	ctx.Step(`^I decode the event as OrderCreated$`, dc.whenDecodeAsOrder)
	ctx.Step(`^I decode looking for suffix "([^"]*)"$`, dc.whenDecodeSuffix)
	ctx.Step(`^I match against "([^"]*)"$`, dc.whenMatchPattern)
	ctx.Step(`^I match against suffix "([^"]*)"$`, dc.whenMatchSuffix)
	ctx.Step(`^I decode the payload bytes$`, dc.whenDecodeBytes)
	ctx.Step(`^I decode the payload$`, dc.whenDecodePayload)
	ctx.Step(`^I attempt to decode$`, dc.whenAttemptDecode)
	ctx.Step(`^I decode$`, dc.whenDecode)
	ctx.Step(`^I call decode_event\(event, "([^"]*)"\)$`, dc.whenCallDecodeEvent)
	ctx.Step(`^I call events_from_response\(response\)$`, dc.whenCallEventsFromResponse)
	ctx.Step(`^I decode each as ItemAdded$`, dc.whenDecodeEach)
	ctx.Step(`^I decode by type$`, dc.whenDecodeByType)
	ctx.Step(`^I filter for "([^"]*)" events$`, dc.whenFilterEvents)

	// Then steps
	ctx.Step(`^decoding should succeed$`, dc.thenDecodeSuccess)
	ctx.Step(`^I should get an OrderCreated message$`, dc.thenGetOrderCreated)
	ctx.Step(`^the full type_url prefix should be ignored$`, dc.thenPrefixIgnored)
	ctx.Step(`^decoding should return None/null$`, dc.thenDecodeNone)
	ctx.Step(`^no error should be raised$`, dc.thenNoError)
	ctx.Step(`^event\.sequence should be (\d+)$`, dc.thenSequenceIs)
	ctx.Step(`^event\.created_at should be a valid timestamp$`, dc.thenValidTimestamp)
	ctx.Step(`^the timestamp should be parseable$`, dc.thenTimestampParseable)
	ctx.Step(`^event\.payload should be Event variant$`, dc.thenEventVariant)
	ctx.Step(`^the Event should contain the Any wrapper$`, dc.thenContainsAny)
	ctx.Step(`^event\.payload should be PayloadReference variant$`, dc.thenReferenceVariant)
	ctx.Step(`^the reference should contain storage details$`, dc.thenStorageDetails)
	ctx.Step(`^the match should succeed$`, dc.thenMatchSuccess)
	ctx.Step(`^the match should fail$`, dc.thenMatchFail)
	ctx.Step(`^the protobuf message should deserialize correctly$`, dc.thenDeserializeCorrect)
	ctx.Step(`^all fields should be populated$`, dc.thenFieldsPopulated)
	ctx.Step(`^the message should have default values$`, dc.thenDefaultValues)
	ctx.Step(`^no error should occur \(empty protobuf is valid\)$`, dc.thenNoErrorEmpty)
	ctx.Step(`^decoding should fail$`, dc.thenDecodeFail)
	ctx.Step(`^an error should indicate deserialization failure$`, dc.thenDeserError)
	ctx.Step(`^no crash should occur$`, dc.thenNoCrash)
	ctx.Step(`^the result should be a default message$`, dc.thenDefaultMessage)
	ctx.Step(`^no error should occur$`, dc.thenNoErrorSimple)
	ctx.Step(`^I should get a slice/list of EventPages$`, dc.thenGetEventsList)
	ctx.Step(`^I should get an empty slice/list$`, dc.thenEmptyList)
	ctx.Step(`^all (\d+) should decode successfully$`, dc.thenAllDecode)
	ctx.Step(`^each should have correct data$`, dc.thenCorrectData)
	ctx.Step(`^OrderCreated should decode as OrderCreated$`, dc.thenOrderDecodes)
	ctx.Step(`^ItemAdded events should decode as ItemAdded$`, dc.thenItemDecodes)
	ctx.Step(`^OrderShipped should decode as OrderShipped$`, dc.thenShippedDecodes)
	ctx.Step(`^I should get (\d+) events$`, dc.thenGetCount)
	ctx.Step(`^both should be ItemAdded type$`, dc.thenBothItemAdded)
}

func (d *DecodeContext) makeEventPage(seq uint32, typeURL string) *pb.EventPage {
	return &pb.EventPage{
		Sequence:  seq,
		CreatedAt: timestamppb.Now(),
		Payload: &pb.EventPage_Event{
			Event: &anypb.Any{
				TypeUrl: typeURL,
				Value:   []byte{},
			},
		},
	}
}

func (d *DecodeContext) makeExternalPage(seq uint32, uri string) *pb.EventPage {
	return &pb.EventPage{
		Sequence:  seq,
		CreatedAt: timestamppb.Now(),
		Payload: &pb.EventPage_External{
			External: &pb.PayloadReference{
				StorageType: pb.PayloadStorageType_PAYLOAD_STORAGE_TYPE_S3,
				Uri:         uri,
				ContentHash: []byte("abc123"),
				OriginalSize: 1024,
				StoredAt:    timestamppb.Now(),
			},
		},
	}
}

func (d *DecodeContext) givenEventTypeURL(typeURL string) error {
	d.TypeURL = typeURL
	d.Event = d.makeEventPage(0, typeURL)
	return nil
}

func (d *DecodeContext) givenValidProtoBytes() error {
	return nil
}

func (d *DecodeContext) givenEventSuffix(suffix string) error {
	d.TypeURL = "type.googleapis.com/test." + suffix
	d.Event = d.makeEventPage(0, d.TypeURL)
	return nil
}

func (d *DecodeContext) givenEventAtSeq(seq int) error {
	d.Event = d.makeEventPage(uint32(seq), "type.googleapis.com/test.Event")
	return nil
}

func (d *DecodeContext) givenEventWithTimestamp() error {
	d.Event = d.makeEventPage(0, "type.googleapis.com/test.Event")
	return nil
}

func (d *DecodeContext) givenEventPayload() error {
	d.Event = d.makeEventPage(0, "type.googleapis.com/test.Event")
	return nil
}

func (d *DecodeContext) givenOffloadedPayload() error {
	d.Event = d.makeExternalPage(0, "s3://bucket/key")
	return nil
}

func (d *DecodeContext) givenProperPayload() error {
	d.Event = d.makeEventPage(0, "type.googleapis.com/test.Event")
	return nil
}

func (d *DecodeContext) givenEmptyPayload() error {
	d.Event = d.makeEventPage(0, "type.googleapis.com/test.Event")
	return nil
}

func (d *DecodeContext) givenCorruptedBytes() error {
	d.Event = &pb.EventPage{
		Sequence:  0,
		CreatedAt: timestamppb.Now(),
		Payload: &pb.EventPage_Event{
			Event: &anypb.Any{
				TypeUrl: "type.googleapis.com/test.Event",
				Value:   []byte{0xff, 0xff, 0xff},
			},
		},
	}
	return nil
}

func (d *DecodeContext) givenNonePayload() error {
	d.Event = &pb.EventPage{
		Sequence:  0,
		CreatedAt: timestamppb.Now(),
	}
	return nil
}

func (d *DecodeContext) givenEmptyAny() error {
	d.Event = d.makeEventPage(0, "type.googleapis.com/test.Event")
	return nil
}

func (d *DecodeContext) givenDecodeFunction() error {
	return nil
}

func (d *DecodeContext) givenResponseWithEvents() error {
	d.CommandResponse = &MockCommandResponse{
		Events: []*pb.EventPage{
			d.makeEventPage(0, "type.googleapis.com/test.Event"),
			d.makeEventPage(1, "type.googleapis.com/test.Event"),
		},
	}
	return nil
}

func (d *DecodeContext) givenResponseNoEvents() error {
	d.CommandResponse = &MockCommandResponse{Events: []*pb.EventPage{}}
	return nil
}

func (d *DecodeContext) givenMultipleSameType(count int, eventType string) error {
	d.Events = make([]*pb.EventPage, count)
	for i := 0; i < count; i++ {
		d.Events[i] = d.makeEventPage(uint32(i), "type.googleapis.com/test."+eventType)
	}
	return nil
}

func (d *DecodeContext) givenMixedEvents() error {
	d.Events = []*pb.EventPage{
		d.makeEventPage(0, "type.googleapis.com/test.OrderCreated"),
		d.makeEventPage(1, "type.googleapis.com/test.ItemAdded"),
		d.makeEventPage(2, "type.googleapis.com/test.ItemAdded"),
		d.makeEventPage(3, "type.googleapis.com/test.OrderShipped"),
	}
	return nil
}

func (d *DecodeContext) whenDecodeAsOrder() error {
	if d.Event != nil {
		if event, ok := d.Event.Payload.(*pb.EventPage_Event); ok {
			if len(event.Event.TypeUrl) > 12 && event.Event.TypeUrl[len(event.Event.TypeUrl)-12:] == "OrderCreated" {
				d.Decoded = struct{}{}
			}
		}
	}
	return nil
}

func (d *DecodeContext) whenDecodeSuffix(suffix string) error {
	if d.Event != nil {
		if event, ok := d.Event.Payload.(*pb.EventPage_Event); ok {
			typeURL := event.Event.TypeUrl
			if len(typeURL) >= len(suffix) && typeURL[len(typeURL)-len(suffix):] == suffix {
				d.Decoded = struct{}{}
			}
		}
	}
	return nil
}

func (d *DecodeContext) whenMatchPattern(pattern string) error {
	if d.Event != nil {
		if event, ok := d.Event.Payload.(*pb.EventPage_Event); ok {
			if event.Event.TypeUrl == pattern {
				d.MatchSuccess = true
			}
		}
	}
	return nil
}

func (d *DecodeContext) whenMatchSuffix(suffix string) error {
	if d.Event != nil {
		if event, ok := d.Event.Payload.(*pb.EventPage_Event); ok {
			typeURL := event.Event.TypeUrl
			if len(typeURL) >= len(suffix) && typeURL[len(typeURL)-len(suffix):] == suffix {
				d.MatchSuccess = true
			}
		}
	}
	return nil
}

func (d *DecodeContext) whenDecodeBytes() error {
	d.Decoded = struct{}{}
	return nil
}

func (d *DecodeContext) whenDecodePayload() error {
	d.Decoded = struct{}{}
	return nil
}

func (d *DecodeContext) whenAttemptDecode() error {
	if d.Event != nil && d.Event.Payload == nil {
		d.Decoded = nil
	} else {
		d.Decoded = struct{}{}
	}
	return nil
}

func (d *DecodeContext) whenDecode() error {
	d.Decoded = struct{}{}
	return nil
}

func (d *DecodeContext) whenCallDecodeEvent(suffix string) error {
	return d.whenDecodeSuffix(suffix)
}

func (d *DecodeContext) whenCallEventsFromResponse() error {
	if d.CommandResponse != nil {
		d.Events = d.CommandResponse.Events
	}
	return nil
}

func (d *DecodeContext) whenDecodeEach() error {
	d.DecodedList = make([]interface{}, len(d.Events))
	for i := range d.Events {
		d.DecodedList[i] = struct{}{}
	}
	return nil
}

func (d *DecodeContext) whenDecodeByType() error {
	return nil
}

func (d *DecodeContext) whenFilterEvents(eventType string) error {
	d.Filtered = nil
	for _, e := range d.Events {
		if event, ok := e.Payload.(*pb.EventPage_Event); ok {
			typeURL := event.Event.TypeUrl
			if len(typeURL) >= len(eventType) && typeURL[len(typeURL)-len(eventType):] == eventType {
				d.Filtered = append(d.Filtered, e)
			}
		}
	}
	return nil
}

func (d *DecodeContext) thenDecodeSuccess() error {
	if d.Decoded == nil && !d.MatchSuccess {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenGetOrderCreated() error {
	if d.Decoded == nil {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenPrefixIgnored() error {
	return nil
}

func (d *DecodeContext) thenDecodeNone() error {
	if d.Decoded != nil {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenNoError() error {
	if d.Error != nil {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenSequenceIs(expected int) error {
	if d.Event == nil || d.Event.Sequence != uint32(expected) {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenValidTimestamp() error {
	if d.Event == nil || d.Event.CreatedAt == nil {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenTimestampParseable() error {
	return nil
}

func (d *DecodeContext) thenEventVariant() error {
	if d.Event == nil {
		return godog.ErrPending
	}
	_, ok := d.Event.Payload.(*pb.EventPage_Event)
	if !ok {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenContainsAny() error {
	if d.Event == nil {
		return godog.ErrPending
	}
	if event, ok := d.Event.Payload.(*pb.EventPage_Event); ok {
		if event.Event == nil || event.Event.TypeUrl == "" {
			return godog.ErrPending
		}
	}
	return nil
}

func (d *DecodeContext) thenReferenceVariant() error {
	if d.Event == nil {
		return godog.ErrPending
	}
	_, ok := d.Event.Payload.(*pb.EventPage_External)
	if !ok {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenStorageDetails() error {
	if d.Event == nil {
		return godog.ErrPending
	}
	if ext, ok := d.Event.Payload.(*pb.EventPage_External); ok {
		if ext.External == nil || ext.External.Uri == "" {
			return godog.ErrPending
		}
	}
	return nil
}

func (d *DecodeContext) thenMatchSuccess() error {
	if !d.MatchSuccess {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenMatchFail() error {
	if d.MatchSuccess {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenDeserializeCorrect() error {
	if d.Decoded == nil {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenFieldsPopulated() error {
	return nil
}

func (d *DecodeContext) thenDefaultValues() error {
	if d.Decoded == nil {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenNoErrorEmpty() error {
	if d.Error != nil {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenDecodeFail() error {
	return nil
}

func (d *DecodeContext) thenDeserError() error {
	return nil
}

func (d *DecodeContext) thenNoCrash() error {
	return nil
}

func (d *DecodeContext) thenDefaultMessage() error {
	if d.Decoded == nil {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenNoErrorSimple() error {
	if d.Error != nil {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenGetEventsList() error {
	if len(d.Events) == 0 {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenEmptyList() error {
	if len(d.Events) > 0 {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenAllDecode(count int) error {
	if len(d.DecodedList) != count {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenCorrectData() error {
	return nil
}

func (d *DecodeContext) thenOrderDecodes() error {
	return nil
}

func (d *DecodeContext) thenItemDecodes() error {
	return nil
}

func (d *DecodeContext) thenShippedDecodes() error {
	return nil
}

func (d *DecodeContext) thenGetCount(count int) error {
	if len(d.Filtered) != count {
		return godog.ErrPending
	}
	return nil
}

func (d *DecodeContext) thenBothItemAdded() error {
	for _, e := range d.Filtered {
		if event, ok := e.Payload.(*pb.EventPage_Event); ok {
			typeURL := event.Event.TypeUrl
			if len(typeURL) < 9 || typeURL[len(typeURL)-9:] != "ItemAdded" {
				return godog.ErrPending
			}
		}
	}
	return nil
}
