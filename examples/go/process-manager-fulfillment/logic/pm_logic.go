// Package logic implements the order-fulfillment process manager: a fan-in
// coordinator that tracks three prerequisites (payment, inventory, packing)
// and issues a Ship command to the fulfillment domain when all are complete.
//
// Internal events (PrerequisiteCompleted, DispatchIssued) are stored as
// JSON-encoded anypb.Any values in process manager state for replay.
package logic

import (
	"encoding/json"
	"fmt"
	"strings"

	angzarrpb "angzarr/proto/angzarr"
	"angzarr/proto/examples"

	"github.com/google/uuid"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

const (
	PMName   = "order-fulfillment"
	PMDomain = "order-fulfillment"

	fulfillmentDomain = "fulfillment"

	prereqPayment     = "payment"
	prereqInventory   = "inventory"
	prereqFulfillment = "fulfillment"

	typeURLPrerequisiteCompleted = "type.examples/examples.PrerequisiteCompleted"
	typeURLDispatchIssued        = "type.examples/examples.DispatchIssued"

	dispatchedMarker = "__dispatched__"
)

var allPrerequisites = []string{prereqPayment, prereqInventory, prereqFulfillment}

// prerequisiteCompleted is stored in PM state when a prerequisite is satisfied.
type prerequisiteCompleted struct {
	Prerequisite string   `json:"prerequisite"`
	Completed    []string `json:"completed"`
	Remaining    []string `json:"remaining"`
}

// dispatchIssued is stored in PM state when all prerequisites are met.
type dispatchIssued struct {
	Completed []string `json:"completed"`
}

// Handle is the PMHandleFunc for the order-fulfillment process manager.
//
// It classifies the trigger event, replays process state to determine which
// prerequisites are already satisfied, and emits a Ship command when all three
// prerequisites are complete. DispatchIssued in state prevents duplicate dispatch.
func Handle(trigger *angzarrpb.EventBook, processState *angzarrpb.EventBook, _ []*angzarrpb.EventBook) ([]*angzarrpb.CommandBook, *angzarrpb.EventBook) {
	correlationID := ""
	if c := trigger.GetCover(); c != nil {
		correlationID = c.CorrelationId
	}
	if correlationID == "" {
		return nil, nil
	}

	completed := extractCompleted(processState)

	if alreadyDispatched(completed) {
		return nil, nil
	}

	var newPrereq string
	for _, page := range trigger.GetPages() {
		if page.GetEvent() == nil {
			continue
		}
		prereq := classifyEvent(page.GetEvent())
		if prereq == "" {
			continue
		}
		if !contains(completed, prereq) {
			completed = append(completed, prereq)
			newPrereq = prereq
		}
	}

	if newPrereq == "" {
		return nil, nil
	}

	pmRootUUID := uuid.NewSHA1(uuid.NameSpaceOID, []byte(correlationID))
	pmRoot := &angzarrpb.UUID{
		Value: pmRootUUID[:],
	}

	nextSeq := nextSequence(processState)

	var pmPages []*angzarrpb.EventPage
	var commands []*angzarrpb.CommandBook

	remaining := difference(allPrerequisites, completed)
	prereqEvt := prerequisiteCompleted{
		Prerequisite: newPrereq,
		Completed:    copyStrings(completed),
		Remaining:    remaining,
	}
	prereqBytes, _ := json.Marshal(prereqEvt)

	pmPages = append(pmPages, &angzarrpb.EventPage{
		Sequence:  &angzarrpb.EventPage_Num{Num: nextSeq},
		CreatedAt: timestamppb.Now(),
		Event: &anypb.Any{
			TypeUrl: typeURLPrerequisiteCompleted,
			Value:   prereqBytes,
		},
	})

	if allComplete(completed) {
		dispatchEvt := dispatchIssued{
			Completed: copyStrings(completed),
		}
		dispatchBytes, _ := json.Marshal(dispatchEvt)

		pmPages = append(pmPages, &angzarrpb.EventPage{
			Sequence:  &angzarrpb.EventPage_Num{Num: nextSeq + 1},
			CreatedAt: timestamppb.Now(),
			Event: &anypb.Any{
				TypeUrl: typeURLDispatchIssued,
				Value:   dispatchBytes,
			},
		})

		orderID := rootIDAsString(trigger.GetCover().GetRoot())

		shipCmd := &examples.Ship{
			Carrier:        fmt.Sprintf("auto-%s", orderID),
			TrackingNumber: "",
		}
		shipAny, err := anypb.New(shipCmd)
		if err != nil {
			return nil, nil
		}

		commands = append(commands, &angzarrpb.CommandBook{
			Cover: &angzarrpb.Cover{
				Domain:        fulfillmentDomain,
				Root:          trigger.GetCover().GetRoot(),
				CorrelationId: correlationID,
			},
			Pages: []*angzarrpb.CommandPage{
				{Command: shipAny},
			},
		})
	}

	pmEventBook := &angzarrpb.EventBook{
		Cover: &angzarrpb.Cover{
			Domain:        PMDomain,
			Root:          pmRoot,
			CorrelationId: correlationID,
		},
		Pages: pmPages,
	}

	return commands, pmEventBook
}

// classifyEvent maps a trigger event type_url to its prerequisite name.
func classifyEvent(event *anypb.Any) string {
	url := event.GetTypeUrl()
	switch {
	case strings.HasSuffix(url, "PaymentSubmitted"):
		return prereqPayment
	case strings.HasSuffix(url, "StockReserved"):
		return prereqInventory
	case strings.HasSuffix(url, "ItemsPacked"):
		return prereqFulfillment
	default:
		return ""
	}
}

// extractCompleted replays process state pages and returns the set of completed
// prerequisite names. If DispatchIssued is found, the dispatchedMarker is included.
func extractCompleted(processState *angzarrpb.EventBook) []string {
	if processState == nil {
		return nil
	}

	var completed []string
	for _, page := range processState.GetPages() {
		event := page.GetEvent()
		if event == nil {
			continue
		}
		url := event.GetTypeUrl()
		if strings.HasSuffix(url, "PrerequisiteCompleted") {
			var evt prerequisiteCompleted
			if err := json.Unmarshal(event.Value, &evt); err != nil {
				continue
			}
			if !contains(completed, evt.Prerequisite) {
				completed = append(completed, evt.Prerequisite)
			}
		} else if strings.HasSuffix(url, "DispatchIssued") {
			if !contains(completed, dispatchedMarker) {
				completed = append(completed, dispatchedMarker)
			}
		}
	}
	return completed
}

// allComplete returns true when all three prerequisites are present.
func allComplete(completed []string) bool {
	for _, p := range allPrerequisites {
		if !contains(completed, p) {
			return false
		}
	}
	return true
}

// alreadyDispatched returns true when the dispatchedMarker is present.
func alreadyDispatched(completed []string) bool {
	return contains(completed, dispatchedMarker)
}

// nextSequence returns the next sequence number for a new PM event page.
func nextSequence(processState *angzarrpb.EventBook) uint32 {
	if processState == nil {
		return 0
	}
	pages := processState.GetPages()
	if len(pages) == 0 {
		return 0
	}
	last := pages[len(pages)-1]
	if num, ok := last.GetSequence().(*angzarrpb.EventPage_Num); ok {
		return num.Num + 1
	}
	return 0
}

// rootIDAsString converts a proto UUID to a human-readable string (or "unknown").
func rootIDAsString(root *angzarrpb.UUID) string {
	if root == nil || len(root.Value) != 16 {
		return "unknown"
	}
	u, err := uuid.FromBytes(root.Value)
	if err != nil {
		return "unknown"
	}
	return u.String()
}

// contains checks if a string slice includes a value.
func contains(slice []string, val string) bool {
	for _, s := range slice {
		if s == val {
			return true
		}
	}
	return false
}

// difference returns elements in all that are not in completed.
func difference(all, completed []string) []string {
	var result []string
	for _, a := range all {
		if !contains(completed, a) {
			result = append(result, a)
		}
	}
	return result
}

// copyStrings returns a shallow copy of a string slice.
func copyStrings(s []string) []string {
	out := make([]string, len(s))
	copy(out, s)
	return out
}
