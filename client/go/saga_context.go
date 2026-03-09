package angzarr

import (
	"encoding/hex"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
)

// SagaContext provides access to destination aggregate state for the splitter pattern.
//
// Used when one event triggers commands to multiple aggregates. Provides sequence
// number lookup for optimistic concurrency control.
//
// Example usage:
//
//	func HandleTableSettled(evt *examples.TableSettled, ctx *SagaContext) []*pb.CommandBook {
//	    commands := make([]*pb.CommandBook, 0, len(evt.Payouts))
//	    for _, payout := range evt.Payouts {
//	        seq := ctx.GetSequence("player", payout.PlayerRoot)
//	        cmd := &examples.TransferFunds{PlayerRoot: payout.PlayerRoot, Amount: payout.Amount}
//	        commands = append(commands, NewCommandBook("player", cmd, seq))
//	    }
//	    return commands
//	}
type SagaContext struct {
	destinations map[string]*pb.EventBook
}

// NewSagaContext creates a context from a list of destination EventBooks.
func NewSagaContext(destinationBooks []*pb.EventBook) *SagaContext {
	ctx := &SagaContext{
		destinations: make(map[string]*pb.EventBook),
	}
	for _, book := range destinationBooks {
		if book.GetCover() != nil && book.GetCover().GetDomain() != "" {
			key := makeSagaContextKey(book.GetCover().GetDomain(), book.GetCover().GetRoot().GetValue())
			ctx.destinations[key] = book
		}
	}
	return ctx
}

// GetSequence returns the next sequence number for a destination aggregate.
// Returns 1 if the aggregate doesn't exist yet.
func (ctx *SagaContext) GetSequence(domain string, aggregateRoot []byte) uint32 {
	key := makeSagaContextKey(domain, aggregateRoot)
	book, ok := ctx.destinations[key]
	if !ok || len(book.GetPages()) == 0 {
		return 1
	}
	lastPage := book.GetPages()[len(book.GetPages())-1]
	if header := lastPage.GetHeader(); header != nil {
		return header.GetSequence() + 1
	}
	return 1
}

// GetDestination returns the EventBook for a destination aggregate, if available.
func (ctx *SagaContext) GetDestination(domain string, aggregateRoot []byte) *pb.EventBook {
	key := makeSagaContextKey(domain, aggregateRoot)
	return ctx.destinations[key]
}

// HasDestination checks if a destination exists.
func (ctx *SagaContext) HasDestination(domain string, aggregateRoot []byte) bool {
	key := makeSagaContextKey(domain, aggregateRoot)
	_, ok := ctx.destinations[key]
	return ok
}

func makeSagaContextKey(domain string, root []byte) string {
	return domain + ":" + hex.EncodeToString(root)
}
