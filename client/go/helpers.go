package angzarr

import (
	"encoding/hex"
	"fmt"
	"strings"
	"time"

	"github.com/google/uuid"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// Constants matching Rust proto_ext::constants
const (
	UnknownDomain         = "unknown"
	WildcardDomain        = "*"
	DefaultEdition        = "angzarr"
	MetaAngzarrDomain     = "_angzarr"
	ProjectionDomainPrefix = "projection:"
	CorrelationIDHeader   = "x-correlation-id"
	TypeURLPrefix         = "type.googleapis.com/"
)

// Cover accessors - work with any type that has a Cover field

// CoverOf extracts the Cover from various proto types.
func CoverOf(v interface{}) *pb.Cover {
	switch t := v.(type) {
	case *pb.EventBook:
		return t.GetCover()
	case *pb.CommandBook:
		return t.GetCover()
	case *pb.Query:
		return t.GetCover()
	case *pb.Cover:
		return t
	default:
		return nil
	}
}

// Domain returns the domain from a Cover-bearing type, or UnknownDomain if missing.
func Domain(v interface{}) string {
	c := CoverOf(v)
	if c == nil || c.Domain == "" {
		return UnknownDomain
	}
	return c.Domain
}

// CorrelationID returns the correlation_id from a Cover-bearing type, or empty if missing.
func CorrelationID(v interface{}) string {
	c := CoverOf(v)
	if c == nil {
		return ""
	}
	return c.CorrelationId
}

// HasCorrelationID returns true if the correlation_id is present and non-empty.
func HasCorrelationID(v interface{}) bool {
	return CorrelationID(v) != ""
}

// RootUUID extracts the root UUID from a Cover-bearing type.
func RootUUID(v interface{}) (uuid.UUID, bool) {
	c := CoverOf(v)
	if c == nil || c.Root == nil {
		return uuid.UUID{}, false
	}
	u, err := uuid.FromBytes(c.Root.Value)
	if err != nil {
		return uuid.UUID{}, false
	}
	return u, true
}

// RootIDHex returns the root UUID as a hex string, or empty if missing.
func RootIDHex(v interface{}) string {
	c := CoverOf(v)
	if c == nil || c.Root == nil {
		return ""
	}
	return hex.EncodeToString(c.Root.Value)
}

// Edition returns the edition name from a Cover-bearing type, defaulting to DefaultEdition.
func Edition(v interface{}) string {
	c := CoverOf(v)
	if c == nil || c.Edition == nil || c.Edition.Name == "" {
		return DefaultEdition
	}
	return c.Edition.Name
}

// EditionOpt returns the edition name as a pointer, nil if not set.
func EditionOpt(v interface{}) *string {
	c := CoverOf(v)
	if c == nil || c.Edition == nil || c.Edition.Name == "" {
		return nil
	}
	return &c.Edition.Name
}

// RoutingKey computes the bus routing key for a Cover-bearing type.
func RoutingKey(v interface{}) string {
	return Domain(v)
}

// CacheKey generates a cache key based on domain + root.
func CacheKey(v interface{}) string {
	return fmt.Sprintf("%s:%s", Domain(v), RootIDHex(v))
}

// UUID conversion

// UUIDToProto converts a uuid.UUID to a proto UUID.
func UUIDToProto(u uuid.UUID) *pb.UUID {
	return &pb.UUID{Value: u[:]}
}

// ProtoToUUID converts a proto UUID to uuid.UUID.
func ProtoToUUID(u *pb.UUID) (uuid.UUID, error) {
	if u == nil {
		return uuid.UUID{}, fmt.Errorf("nil UUID")
	}
	return uuid.FromBytes(u.Value)
}

// BytesToUUIDText converts bytes to standard UUID text format.
// If bytes are exactly 16 bytes, formats as UUID (8-4-4-4-12).
// Otherwise returns hex encoding of the bytes.
func BytesToUUIDText(b []byte) string {
	if len(b) == 16 {
		u, err := uuid.FromBytes(b)
		if err == nil {
			return u.String()
		}
	}
	return hex.EncodeToString(b)
}

// ProtoUUIDToText converts a proto UUID to text format.
func ProtoUUIDToText(u *pb.UUID) string {
	if u == nil {
		return ""
	}
	return BytesToUUIDText(u.Value)
}

// RootIDText returns the root UUID as standard text format (8-4-4-4-12), or empty if missing.
func RootIDText(v interface{}) string {
	c := CoverOf(v)
	if c == nil || c.Root == nil {
		return ""
	}
	return BytesToUUIDText(c.Root.Value)
}

// Edition helpers

// MainTimeline returns an Edition representing the main timeline.
func MainTimeline() *pb.Edition {
	return &pb.Edition{Name: DefaultEdition}
}

// ImplicitEdition creates an edition with the given name but no divergences.
func ImplicitEdition(name string) *pb.Edition {
	return &pb.Edition{Name: name}
}

// ExplicitEdition creates an edition with divergence points.
func ExplicitEdition(name string, divergences []*pb.DomainDivergence) *pb.Edition {
	return &pb.Edition{Name: name, Divergences: divergences}
}

// IsMainTimeline checks if an edition represents the main timeline.
func IsMainTimeline(e *pb.Edition) bool {
	return e == nil || e.Name == "" || e.Name == DefaultEdition
}

// DivergenceFor returns the divergence sequence for a domain, or -1 if not found.
func DivergenceFor(e *pb.Edition, domain string) int64 {
	if e == nil {
		return -1
	}
	for _, d := range e.Divergences {
		if d.Domain == domain {
			return int64(d.Sequence)
		}
	}
	return -1
}

// EventBook helpers

// NextSequence returns the next sequence number from an EventBook.
// The framework computes this value on load.
func NextSequence(book *pb.EventBook) uint32 {
	if book == nil {
		return 0
	}
	return book.NextSequence
}

// EventPages returns the event pages from an EventBook, or empty slice if nil.
func EventPages(book *pb.EventBook) []*pb.EventPage {
	if book == nil {
		return nil
	}
	return book.Pages
}

// CommandBook helpers

// CommandPages returns the command pages from a CommandBook, or empty slice if nil.
func CommandPages(book *pb.CommandBook) []*pb.CommandPage {
	if book == nil {
		return nil
	}
	return book.Pages
}

// CommandResponse helpers

// EventsFromResponse extracts the event pages from a CommandResponse.
func EventsFromResponse(resp *pb.CommandResponse) []*pb.EventPage {
	if resp == nil || resp.Events == nil {
		return nil
	}
	return resp.Events.Pages
}

// Type URL helpers

// TypeURL constructs a full type URL from a package and type name.
func TypeURL(packageName, typeName string) string {
	return TypeURLPrefix + packageName + "." + typeName
}

// TypeNameFromURL extracts the type name from a type URL.
func TypeNameFromURL(typeURL string) string {
	if idx := strings.LastIndex(typeURL, "."); idx >= 0 {
		return typeURL[idx+1:]
	}
	if idx := strings.LastIndex(typeURL, "/"); idx >= 0 {
		return typeURL[idx+1:]
	}
	return typeURL
}

// TypeURLMatches checks if a type URL ends with the given suffix.
func TypeURLMatches(typeURL, suffix string) bool {
	return strings.HasSuffix(typeURL, suffix)
}

// Timestamp helpers

// Now returns the current time as a protobuf Timestamp.
func Now() *timestamppb.Timestamp {
	return timestamppb.Now()
}

// ParseTimestamp parses an RFC3339 timestamp string.
func ParseTimestamp(rfc3339 string) (*timestamppb.Timestamp, error) {
	t, err := time.Parse(time.RFC3339, rfc3339)
	if err != nil {
		return nil, InvalidTimestampError(err.Error())
	}
	return timestamppb.New(t), nil
}

// Event decoding

// DecodeEvent attempts to decode an event payload if the type URL matches.
func DecodeEvent(page *pb.EventPage, typeSuffix string, msg interface{ Unmarshal([]byte) error }) bool {
	if page == nil || page.Event == nil {
		return false
	}
	if !TypeURLMatches(page.Event.TypeUrl, typeSuffix) {
		return false
	}
	return msg.Unmarshal(page.Event.Value) == nil
}

// NewCover creates a new Cover with the given parameters.
func NewCover(domain string, root uuid.UUID, correlationID string) *pb.Cover {
	return &pb.Cover{
		Domain:        domain,
		Root:          UUIDToProto(root),
		CorrelationId: correlationID,
	}
}

// NewCoverWithEdition creates a Cover with an edition.
func NewCoverWithEdition(domain string, root uuid.UUID, correlationID string, edition *pb.Edition) *pb.Cover {
	return &pb.Cover{
		Domain:        domain,
		Root:          UUIDToProto(root),
		CorrelationId: correlationID,
		Edition:       edition,
	}
}

// NewCommandPage creates a command page from a sequence and Any message.
func NewCommandPage(sequence uint32, command *anypb.Any) *pb.CommandPage {
	return &pb.CommandPage{
		Sequence: sequence,
		Command:  command,
	}
}

// NewCommandBook creates a CommandBook with a single command.
func NewCommandBook(cover *pb.Cover, pages ...*pb.CommandPage) *pb.CommandBook {
	return &pb.CommandBook{
		Cover: cover,
		Pages: pages,
	}
}

// NewQueryWithRange creates a Query with a cover and range selection.
func NewQueryWithRange(cover *pb.Cover, lower uint32, upper *uint32) *pb.Query {
	r := &pb.SequenceRange{Lower: lower}
	if upper != nil {
		r.Upper = upper
	}
	return &pb.Query{
		Cover:     cover,
		Selection: &pb.Query_Range{Range: r},
	}
}

// NewQueryWithTemporal creates a Query with a temporal selection.
func NewQueryWithTemporal(cover *pb.Cover, temporal *pb.TemporalQuery) *pb.Query {
	return &pb.Query{
		Cover:     cover,
		Selection: &pb.Query_Temporal{Temporal: temporal},
	}
}

// RangeSelection creates a sequence range selection (returns the oneof wrapper).
func RangeSelection(lower uint32, upper *uint32) *pb.Query_Range {
	r := &pb.SequenceRange{Lower: lower}
	if upper != nil {
		r.Upper = upper
	}
	return &pb.Query_Range{Range: r}
}

// TemporalSelectionBySequence creates a temporal selection as-of a sequence.
func TemporalSelectionBySequence(seq uint32) *pb.Query_Temporal {
	return &pb.Query_Temporal{
		Temporal: &pb.TemporalQuery{
			PointInTime: &pb.TemporalQuery_AsOfSequence{AsOfSequence: seq},
		},
	}
}

// TemporalSelectionByTime creates a temporal selection as-of a timestamp.
func TemporalSelectionByTime(ts *timestamppb.Timestamp) *pb.Query_Temporal {
	return &pb.Query_Temporal{
		Temporal: &pb.TemporalQuery{
			PointInTime: &pb.TemporalQuery_AsOfTime{AsOfTime: ts},
		},
	}
}
