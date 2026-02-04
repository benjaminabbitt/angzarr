// Package angzarr provides shared event logging utilities for projectors.
package angzarr

import (
	"fmt"
	"strings"

	goproto "google.golang.org/protobuf/proto"

	"angzarr/proto/examples"
)

// ANSI color codes
const (
	Blue    = "\033[94m"
	Green   = "\033[92m"
	Yellow  = "\033[93m"
	Cyan    = "\033[96m"
	Magenta = "\033[95m"
	Red     = "\033[91m"
	Bold    = "\033[1m"
	Dim     = "\033[2m"
	Reset   = "\033[0m"
)

// DomainColor returns the color for a domain.
func DomainColor(domain string) string {
	if domain == "customer" {
		return Blue
	}
	return Magenta
}

// EventColor returns the color for an event type.
func EventColor(eventType string) string {
	switch {
	case strings.Contains(eventType, "Created"):
		return Green
	case strings.Contains(eventType, "Completed"):
		return Cyan
	case strings.Contains(eventType, "Cancelled"):
		return Red
	case strings.Contains(eventType, "Added"), strings.Contains(eventType, "Applied"):
		return Yellow
	default:
		return ""
	}
}

// LogEvent logs a single event with pretty formatting.
func LogEvent(domain, rootID string, sequence uint32, typeURL string, data []byte) {
	eventType := typeURL
	if idx := strings.LastIndex(typeURL, "."); idx >= 0 {
		eventType = typeURL[idx+1:]
	}

	// Header
	fmt.Println()
	fmt.Printf("%s%s%s\n", Bold, strings.Repeat("─", 60), Reset)
	fmt.Printf("%s%s[%s]%s %sseq:%d%s  %s%s...%s\n",
		Bold, DomainColor(domain), strings.ToUpper(domain), Reset,
		Dim, sequence, Reset,
		Cyan, rootID, Reset)
	fmt.Printf("%s%s%s%s\n", Bold, EventColor(eventType), eventType, Reset)
	fmt.Println(strings.Repeat("─", 60))

	// Event-specific details
	PrintEventDetails(eventType, data)
}

// PrintEventDetails parses and prints event-specific details.
func PrintEventDetails(eventType string, data []byte) {
	switch eventType {
	case "CustomerCreated":
		var event examples.CustomerCreated
		if err := goproto.Unmarshal(data, &event); err == nil {
			fmt.Printf("  %sname:%s    %s\n", Dim, Reset, event.Name)
			fmt.Printf("  %semail:%s   %s\n", Dim, Reset, event.Email)
			if event.CreatedAt != nil {
				fmt.Printf("  %screated:%s %s\n", Dim, Reset, event.CreatedAt.AsTime().Format("2006-01-02T15:04:05"))
			}
		}

	case "LoyaltyPointsAdded":
		var event examples.LoyaltyPointsAdded
		if err := goproto.Unmarshal(data, &event); err == nil {
			fmt.Printf("  %spoints:%s      +%d\n", Dim, Reset, event.Points)
			fmt.Printf("  %snew_balance:%s %d\n", Dim, Reset, event.NewBalance)
			fmt.Printf("  %sreason:%s      %s\n", Dim, Reset, event.Reason)
		}

	case "LoyaltyPointsRedeemed":
		var event examples.LoyaltyPointsRedeemed
		if err := goproto.Unmarshal(data, &event); err == nil {
			fmt.Printf("  %spoints:%s      -%d\n", Dim, Reset, event.Points)
			fmt.Printf("  %snew_balance:%s %d\n", Dim, Reset, event.NewBalance)
			fmt.Printf("  %stype:%s        %s\n", Dim, Reset, event.RedemptionType)
		}

	case "OrderCreated":
		var event examples.OrderCreated
		if err := goproto.Unmarshal(data, &event); err == nil {
			custID := event.CustomerId
			if len(custID) > 16 {
				custID = custID[:16]
			}
			fmt.Printf("  %scustomer:%s %s...\n", Dim, Reset, custID)
			fmt.Printf("  %sitems:%s\n", Dim, Reset)
			for _, item := range event.Items {
				lineTotal := item.Quantity * item.UnitPriceCents
				fmt.Printf("    - %dx %s @ $%.2f = $%.2f\n",
					item.Quantity, item.Name,
					float64(item.UnitPriceCents)/100,
					float64(lineTotal)/100)
			}
			fmt.Printf("  %ssubtotal:%s $%.2f\n", Dim, Reset, float64(event.SubtotalCents)/100)
		}

	case "LoyaltyDiscountApplied":
		var event examples.LoyaltyDiscountApplied
		if err := goproto.Unmarshal(data, &event); err == nil {
			fmt.Printf("  %spoints:%s  %d\n", Dim, Reset, event.PointsUsed)
			fmt.Printf("  %sdiscount:%s -$%.2f\n", Dim, Reset, float64(event.DiscountCents)/100)
		}

	case "OrderCompleted":
		var event examples.OrderCompleted
		if err := goproto.Unmarshal(data, &event); err == nil {
			fmt.Printf("  %stotal:%s    $%.2f\n", Dim, Reset, float64(event.FinalTotalCents)/100)
			fmt.Printf("  %spayment:%s  %s\n", Dim, Reset, event.PaymentMethod)
			fmt.Printf("  %sloyalty:%s  +%d pts\n", Dim, Reset, event.LoyaltyPointsEarned)
		}

	case "OrderCancelled":
		var event examples.OrderCancelled
		if err := goproto.Unmarshal(data, &event); err == nil {
			fmt.Printf("  %sreason:%s %s\n", Dim, Reset, event.Reason)
		}

	default:
		fmt.Printf("  %s(raw bytes: %d bytes)%s\n", Dim, len(data), Reset)
	}
}
