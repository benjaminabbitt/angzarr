# Aviation Network Recovery System - Implementation Plan

## Executive Summary

An event-sourced system for modeling and recovering an airline's flight network after disruptions (delays, cancellations, airport closures). The system tracks all operational resources, enables "what-if" analysis, and optimizes recovery decisions considering downstream impacts on passengers, cargo, crew, and aircraft.

## Domain Research Summary

### Industry Context

The airline industry loses approximately **$35 billion annually** due to disruptions globally. In the US, 22% of flights are delayed and 3% cancelled since 2001. Airlines use **Airline Operations Control Centers (AOCCs)** to manage Irregular Operations (IROPs), but traditional sequential decision-making leads to suboptimal recovery.

Modern research (AIRS.ACR, 2025) uses **Time-Space Network (TSN)** formulations with mixed-integer linear programming (MILP) to jointly optimize aircraft rotations, crew pairings, maintenance windows, and airport slots.

### Key Sources

- [ScienceDirect: Integrated Recovery](https://www.sciencedirect.com/science/article/abs/pii/S096969979800012X)
- [arXiv: Disruption Management TSN Approach](https://arxiv.org/html/2510.26831)
- [eCFR Part 117: Pilot Duty/Rest](https://www.ecfr.gov/current/title-14/chapter-I/subchapter-G/part-117)
- [BTS: On-Time Performance Data](https://www.transtats.bts.gov/Tables.asp?DB_ID=120)
- [SKYbrary: MEL](https://skybrary.aero/articles/minimum-equipment-list-mel)

---

## Domain Model

### Aggregates (Bounded Contexts)

| Aggregate | Description | Key State |
|-----------|-------------|-----------|
| `flight` | Individual scheduled flight | schedule, status, delays, actual times |
| `aircraft` | Physical airframe | position, maintenance status, MEL items |
| `cockpit-crew` | Pilots (captain, FO) | duty hours, rest, qualifications, position |
| `cabin-crew` | Flight attendants | duty hours, rest, position |
| `passenger` | Individual traveler | itinerary, loyalty tier, connections |
| `cargo` | Freight shipments | SLA, penalties, priority |
| `airport` | Airport resource hub | slots, gates, curfews, ground capacity |

### Sagas (Orchestration)

| Saga | Purpose |
|------|---------|
| `saga-flight-delay` | Propagate delay impact through network |
| `saga-crew-swap` | Reassign crew when duty limits exceeded |
| `saga-aircraft-swap` | Substitute aircraft when unavailable |
| `saga-passenger-rebook` | Find alternatives for disrupted passengers |
| `saga-cargo-reroute` | Reroute freight to meet SLAs |
| `saga-network-recovery` | Orchestrate full recovery optimization |

### Projectors

| Projector | Output |
|-----------|--------|
| `projector-network-state` | Real-time network view |
| `projector-crew-legality` | Crew duty/rest compliance view |
| `projector-passenger-impact` | Disruption impact by passenger |
| `projector-cargo-penalties` | Accrued/projected cargo penalties |
| `projector-loyalty-impact` | Future revenue impact model |

---

## Resource Constraints

### 1. Cockpit Crew (FAR Part 117)

| Constraint | Value |
|------------|-------|
| Rest before duty | 10 hours (min 8 uninterrupted sleep opportunity) |
| Weekly rest | 30 consecutive hours in any 168-hour period |
| Max flight time (2 pilots) | 8-9 hours hard limit |
| Max flight time (3 pilots) | 13 hours (augmented) |
| Max flight time (4 pilots) | 17 hours (augmented) |
| FDP extension | Up to 2 hours (once before rest) |
| Long-haul recovery | 56 hours rest after >60° longitude travel |

### 2. Cabin Crew

Similar duty limits with variations by carrier collective bargaining agreements.

### 3. Aircraft

| Constraint | Description |
|------------|-------------|
| Minimum turnaround | 25-40 min (narrow-body), 60+ min (wide-body) |
| MEL Category A | Manufacturer-specified repair interval |
| MEL Category B | 3 calendar days (72 hours) |
| MEL Category C | 10 calendar days (240 hours) |
| MEL Category D | 120 calendar days |
| Scheduled maintenance | A/B/C/D checks at intervals |

### 4. Airport Slots

| Constraint | Description |
|------------|-------------|
| Slot pair | Landing + takeoff slot |
| Curfews | Night operation restrictions (e.g., 23:00-06:00) |
| Declared capacity | Max movements per hour |
| Ground handling | Min turnaround depends on handler availability |
| Gate availability | Limited gates per terminal |

### 5. Passengers

| Factor | Impact |
|--------|--------|
| Loyalty tier | Priority for rebooking, protection |
| Connection window | Minimum connection time (MCT) |
| Rebooking cost | 1x same airline/day, 2-6x other airline/day |
| Compensation | EU261, DOT rules based on delay duration |
| Future loyalty | 37% switch airlines after major disruption |

### 6. Cargo

| Factor | Description |
|--------|-------------|
| Priority levels | Standard, Priority, Express |
| SLA penalties | Time-based, often per-hour |
| Value of time | ~€13,000/hour per full freighter (2010 baseline) |
| Free storage | Typically 24 hours before charges |

---

## Event Types

### Flight Events
```
FlightScheduled { flight_id, aircraft_id, origin, destination, std, sta }
FlightDelayed { flight_id, new_std, new_sta, delay_minutes, reason_code }
FlightCancelled { flight_id, reason_code }
FlightDeparted { flight_id, atd, gate }
FlightArrived { flight_id, ata, gate }
FlightDiverted { flight_id, diversion_airport, reason }
AircraftAssigned { flight_id, aircraft_id }
AircraftSwapped { flight_id, old_aircraft_id, new_aircraft_id, reason }
```

### Crew Events
```
CrewAssigned { flight_id, crew_id, role }
CrewReassigned { flight_id, old_crew_id, new_crew_id, role, reason }
DutyPeriodStarted { crew_id, report_time, base }
DutyPeriodEnded { crew_id, release_time, base }
RestPeriodStarted { crew_id, start_time, location }
RestPeriodEnded { crew_id, end_time }
CrewPositioned { crew_id, flight_id } // deadhead
```

### Passenger Events
```
BookingCreated { pnr, passenger_id, itinerary, loyalty_tier }
ItineraryDisrupted { passenger_id, affected_flights, disruption_type }
PassengerRebooked { passenger_id, old_itinerary, new_itinerary, method }
PassengerRefunded { passenger_id, amount, reason }
ConnectionMissed { passenger_id, missed_flight_id, reason }
LoyaltyImpactRecorded { passenger_id, impact_score, churn_probability }
```

### Cargo Events
```
CargoBooked { shipment_id, flight_id, weight_kg, priority, sla_deadline }
CargoLoaded { shipment_id, flight_id }
CargoOffloaded { shipment_id, flight_id, reason }
CargoRerouted { shipment_id, old_routing, new_routing }
CargoPenaltyAccrued { shipment_id, penalty_amount, delay_hours }
CargoDelivered { shipment_id, actual_delivery_time }
```

### Airport Events
```
SlotAllocated { airport, flight_id, slot_time, slot_type }
SlotReleased { airport, flight_id, slot_time }
AirportCapacityReduced { airport, new_capacity, reason, duration }
AirportClosed { airport, closure_start, closure_end, reason }
GateAssigned { airport, flight_id, gate, time }
```

---

## Commands

### Operational Commands
```
ScheduleFlight { flight_id, aircraft_id, origin, destination, std, sta }
DelayFlight { flight_id, new_std, reason_code }
CancelFlight { flight_id, reason_code }
SwapAircraft { flight_id, new_aircraft_id }
```

### Crew Commands
```
AssignCrew { flight_id, crew_id, role }
ReassignCrew { flight_id, from_crew_id, to_crew_id }
PositionCrew { crew_id, flight_id } // deadhead
StartDutyPeriod { crew_id, report_time }
EndDutyPeriod { crew_id, release_time }
```

### Passenger Commands
```
RebookPassenger { passenger_id, new_itinerary }
RefundPassenger { passenger_id, amount }
ProtectConnection { passenger_id, connecting_flight_id }
```

### Cargo Commands
```
RerouteCargo { shipment_id, new_routing }
OffloadCargo { shipment_id, flight_id, reason }
PrioritizeCargo { shipment_id, new_priority }
```

### Recovery Commands
```
InjectDisruption { disruption_type, affected_resources, parameters }
OptimizeRecovery { scope, constraints, objective }
SimulateRecovery { recovery_plan, horizon }
```

---

## Cost Model

### Passenger Disruption Costs

| Rebooking Method | Relative Cost |
|------------------|---------------|
| Same airline, same day | 1.0x |
| Same airline, different day | 1.5x |
| Other airline, same day | 3.0x |
| Other airline, different day | 4.0x |
| Full refund | 2.0x + future revenue loss |

### Loyalty Impact Model

```
churn_probability = base_churn_rate * (1 + delay_severity * tier_sensitivity)
future_revenue_loss = lifetime_value * churn_probability

tier_sensitivity:
  - Basic: 1.2 (more likely to churn)
  - Silver: 1.0
  - Gold: 0.8
  - Platinum: 0.5 (invested in program)
```

### Cargo Penalty Model

```
penalty = hourly_rate * delay_hours * priority_multiplier

priority_multiplier:
  - Standard: 1.0
  - Priority: 2.5
  - Express: 5.0
```

### Crew Cost Model

```
overtime_cost = base_hourly * overtime_multiplier * hours
hotel_cost = per_diem + room_rate (if outstation overnight)
deadhead_cost = seat_value + duty_time_consumed
```

---

## Static Data Sources

### Primary: BTS On-Time Performance
- URL: https://www.transtats.bts.gov/Tables.asp?DB_ID=120
- Contains: All US domestic flights since 1987
- Fields: carrier, flight_num, origin, dest, scheduled/actual times, delay causes
- Format: Downloadable CSV/ZIP

### Aircraft Fleet Data
- Manually curated or from aviation databases
- Fields: registration, type, capacity, range, operator

### Airport Data
- Source: OurAirports (open data)
- Fields: IATA/ICAO codes, timezone, coordinates, capacity estimates

### Crew Duty Rules
- Configuration files modeling FAR 117 / EU-OPS FTL rules

---

## Implementation Phases

### Phase 1: Core Aggregates
1. `flight` aggregate with basic lifecycle
2. `aircraft` aggregate with position tracking
3. `cockpit-crew` aggregate with duty tracking
4. Basic event sourcing and state rebuild

### Phase 2: Constraint Modeling
1. FAR Part 117 duty/rest validation
2. Aircraft turnaround constraints
3. MEL item tracking
4. Airport slot/curfew constraints

### Phase 3: Passengers & Cargo
1. `passenger` aggregate with itineraries
2. `cargo` aggregate with SLAs
3. Connection tracking
4. Rebooking/rerouting logic

### Phase 4: Recovery Sagas
1. `saga-flight-delay` for propagation
2. `saga-crew-swap` for reassignment
3. `saga-passenger-rebook` for protection
4. `saga-cargo-reroute` for freight

### Phase 5: Analytics & Simulation
1. Projectors for real-time views
2. Loyalty impact model
3. Cost calculation
4. "What-if" simulation mode

### Phase 6: Data Import
1. BTS data parser
2. Schedule loader
3. Disruption injection CLI
4. Scenario playback

---

## Directory Structure

```
examples/aviation-repair/
├── README.md
├── justfile
├── go.mod
├── Containerfile
├── skaffold.yaml
├── helm/
│   └── values.yaml
├── proto/
│   ├── aviation/
│   │   ├── flight.proto
│   │   ├── aircraft.proto
│   │   ├── crew.proto
│   │   ├── passenger.proto
│   │   ├── cargo.proto
│   │   └── airport.proto
│   └── commands/
│       └── aviation_commands.proto
├── generated/
│   └── (proto output)
├── flight/
│   ├── main.go
│   ├── logic/
│   │   ├── flight_logic.go
│   │   ├── flight_state.go
│   │   └── errors.go
│   └── features/
│       └── flight.feature
├── aircraft/
│   └── (same structure)
├── cockpit-crew/
│   └── (same structure)
├── cabin-crew/
│   └── (same structure)
├── passenger/
│   └── (same structure)
├── cargo/
│   └── (same structure)
├── airport/
│   └── (same structure)
├── saga-flight-delay/
│   └── (saga implementation)
├── saga-crew-swap/
│   └── (saga implementation)
├── saga-passenger-rebook/
│   └── (saga implementation)
├── saga-cargo-reroute/
│   └── (saga implementation)
├── projector-network-state/
│   └── (projector implementation)
├── projector-crew-legality/
│   └── (projector implementation)
├── data/
│   ├── bts_loader.go
│   ├── sample_schedule.json
│   └── README.md
├── simulation/
│   ├── disruption_injector.go
│   └── scenario_runner.go
└── features/
    ├── flight.feature
    ├── aircraft.feature
    ├── cockpit_crew.feature
    ├── cabin_crew.feature
    ├── passenger.feature
    ├── cargo.feature
    ├── saga_delay_propagation.feature
    └── saga_crew_swap.feature
```

---

## Gherkin Scenarios (Examples)

### Flight Delay Propagation
```gherkin
Feature: Flight Delay Propagation
  Tests downstream impact when a flight is delayed.

  Scenario: Delay propagates to connecting flight crew
    Given flight "AA100" scheduled JFK->LAX departing 08:00 arriving 11:00
    And flight "AA200" scheduled LAX->SFO departing 12:00 with crew from AA100
    And the crew minimum rest between flights is 30 minutes
    When flight "AA100" is delayed by 90 minutes
    Then flight "AA200" must be delayed to at least 12:30
    And a CrewConnectionImpact event is recorded

  Scenario: Delay does not propagate when buffer exists
    Given flight "AA100" scheduled JFK->LAX departing 08:00 arriving 11:00
    And flight "AA200" scheduled LAX->SFO departing 14:00 with crew from AA100
    When flight "AA100" is delayed by 60 minutes
    Then flight "AA200" remains at 14:00
```

### Crew Duty Limits
```gherkin
Feature: Crew Duty Compliance
  Tests FAR Part 117 duty and rest requirements.

  Scenario: Crew cannot be assigned if duty limit exceeded
    Given pilot "P001" started duty at 06:00
    And pilot "P001" has no augmentation
    And the current time is 14:30
    When I try to assign pilot "P001" to a 3-hour flight departing 15:00
    Then the assignment is rejected
    And the rejection reason is "would exceed 9-hour flight time limit"

  Scenario: Crew assigned when within limits
    Given pilot "P001" started duty at 06:00
    And pilot "P001" has accumulated 4 hours flight time today
    When I try to assign pilot "P001" to a 3-hour flight departing 11:00
    Then the assignment is accepted
```

### Passenger Rebooking Priority
```gherkin
Feature: Passenger Rebooking
  Tests priority-based rebooking during disruptions.

  Scenario: Platinum passengers rebooked first
    Given passenger "PAX001" with tier "Platinum" on cancelled flight "AA100"
    And passenger "PAX002" with tier "Basic" on cancelled flight "AA100"
    And limited seats on alternative flight "AA101"
    When automatic rebooking runs
    Then passenger "PAX001" is rebooked on "AA101"
    And passenger "PAX002" receives a refund offer
```

### Cargo Penalty Accrual
```gherkin
Feature: Cargo Penalty Tracking
  Tests cargo SLA penalties during delays.

  Scenario: Express cargo accrues penalties after deadline
    Given cargo shipment "CGO001" with priority "Express"
    And SLA deadline of 14:00
    And penalty rate of $500/hour
    When the shipment is delivered at 16:30
    Then penalty accrued is $1250
    And a CargoPenaltyAccrued event is recorded with amount 1250
```

---

## Design Decisions

| Question | Decision |
|----------|----------|
| Optimization Scope | **Full network** - optimize across entire airline network |
| Simulation Speed | **Both** - real-time playback and fast-forward modes |
| Multi-Airline | **Codeshare/interline** - model operating vs marketing carriers |
| Weather/Scenarios | **Static historical** - replay actual closures/delays from BTS data |
| UI | **Web dashboard** - real-time visualization with REST/WebSocket API |

---

## Multi-Airline & Airport Contention Model

### Codeshare/Interline Support

Flights have both operating and marketing carriers:

```
FlightScheduled {
  flight_id,
  operating_carrier,           // Who flies the plane (e.g., AA)
  marketing_carriers[],        // Who sells seats (e.g., AA, BA, QF)
  aircraft_id,
  origin, destination,
  std, sta
}
```

**Interline agreements** define:
- Which carriers accept rebookings from each other
- Cost sharing for disruption handling
- Baggage/connection guarantees

### Airport Traffic Model

To evaluate slot availability, we model **all traffic** at an airport, not just our airline:

```
AirportTrafficEvent {
  airport,
  flight_id,
  carrier,
  is_own_airline: bool,        // false = competitor traffic
  operation_type,              // arrival/departure
  scheduled_time,
  actual_time
}
```

**Slot contention** evaluated when:
- Our recovery solution proposes a new arrival/departure time
- Must check against declared capacity AND existing traffic
- Competing traffic loaded from BTS historical data

### Airport State

```go
type AirportState struct {
  IATA            string
  DeclaredCapacity struct {
    ArrivalsPerHour   int
    DeparturesPerHour int
  }
  Curfew struct {
    Start time.Time  // e.g., 23:00 local
    End   time.Time  // e.g., 06:00 local
  }
  // Slots occupied by time bucket (15-min intervals)
  OccupiedSlots map[TimeSlot][]FlightRef
  // Our airline's slots
  OwnSlots      map[TimeSlot][]FlightRef
  // Gates
  Gates         []Gate
  GateAssignments map[FlightRef]Gate
}
```

---

## Simulation Engine

### Time Abstraction

All components operate against a `SimulationClock` interface:

```go
type SimulationClock interface {
  Now() time.Time
  Advance(d time.Duration)
  SetSpeed(multiplier float64)  // 1.0 = real-time, 60.0 = 1 min/sec
  Subscribe(interval time.Duration, ch chan time.Time)
}
```

### Playback Modes

| Mode | Speed | Use Case |
|------|-------|----------|
| Real-time | 1x | Training, live demo |
| Fast-forward | 60x-3600x | Scenario analysis |
| Step | Manual | Debugging, what-if |
| Instant | Max | Batch optimization |

### Event Replay

Historical scenarios loaded from BTS data:

```go
type ScenarioLoader interface {
  LoadDay(date time.Time) ([]ScheduledEvent, error)
  LoadDisruptions(date time.Time) ([]DisruptionEvent, error)
  LoadAirportTraffic(airport string, date time.Time) ([]TrafficEvent, error)
}
```

Events injected into simulation at their historical timestamps.

---

## Web Dashboard

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Web Dashboard (SPA)                      │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐│
│  │ Network  │ │ Crew     │ │ Passenger│ │ Cost/Penalty     ││
│  │ Map View │ │ Status   │ │ Impact   │ │ Dashboard        ││
│  └──────────┘ └──────────┘ └──────────┘ └──────────────────┘│
└─────────────────────────────────────────────────────────────┘
                            │
                   WebSocket│REST
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                    Dashboard API Server                      │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐│
│  │ REST API │ │ WebSocket│ │ SSE      │ │ Simulation       ││
│  │ /api/v1  │ │ /ws      │ │ /events  │ │ Control          ││
│  └──────────┘ └──────────┘ └──────────┘ └──────────────────┘│
└─────────────────────────────────────────────────────────────┘
                            │
                    Subscribe│to projectors
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                       Projectors                             │
│  ┌──────────────┐ ┌─────────────┐ ┌────────────────────────┐│
│  │ network-state│ │ crew-status │ │ passenger-impact       ││
│  └──────────────┘ └─────────────┘ └────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

### Dashboard Views

1. **Network Map**
   - Geographic view of flights in progress
   - Color-coded by status (on-time, delayed, diverted)
   - Click for flight details
   - Airport status indicators

2. **Crew Status**
   - Duty time remaining per crew member
   - Alerts for approaching limits
   - Position tracking (where crew currently is)

3. **Passenger Impact**
   - Disrupted passengers by flight
   - Rebooking status
   - Loyalty tier breakdown
   - Connection risk alerts

4. **Cost Dashboard**
   - Running total: crew, passenger, cargo
   - Projected penalties
   - Loyalty churn forecast
   - Recovery cost by decision

5. **Simulation Control**
   - Play/pause/step
   - Speed control slider
   - Scenario selection
   - Disruption injection panel

### API Endpoints

```
REST:
  GET  /api/v1/flights                    # All flights
  GET  /api/v1/flights/{id}               # Single flight detail
  GET  /api/v1/airports/{code}/traffic    # Airport traffic view
  GET  /api/v1/crew/{id}/status           # Crew duty status
  GET  /api/v1/passengers/disrupted       # Disrupted passengers
  GET  /api/v1/costs/summary              # Cost rollup
  POST /api/v1/simulation/speed           # Set simulation speed
  POST /api/v1/simulation/inject          # Inject disruption
  POST /api/v1/commands/{type}            # Issue command

WebSocket:
  /ws/network          # Real-time network state updates
  /ws/costs            # Cost ticker
  /ws/alerts           # Crew/connection alerts
```

---

## Updated Directory Structure

```
examples/aviation-repair/
├── README.md
├── justfile
├── go.mod
├── Containerfile
├── skaffold.yaml
├── helm/
│   └── values.yaml
├── proto/
│   ├── aviation/
│   │   ├── flight.proto
│   │   ├── aircraft.proto
│   │   ├── crew.proto
│   │   ├── passenger.proto
│   │   ├── cargo.proto
│   │   ├── airport.proto
│   │   └── interline.proto       # NEW: codeshare agreements
│   └── commands/
│       └── aviation_commands.proto
├── generated/
├── common/
│   ├── clock.go                  # NEW: SimulationClock
│   └── time_bucket.go            # NEW: slot time utilities
├── flight/
├── aircraft/
├── cockpit-crew/
├── cabin-crew/
├── passenger/
├── cargo/
├── airport/
│   ├── logic/
│   │   ├── airport_logic.go
│   │   ├── slot_manager.go       # NEW: slot contention
│   │   └── traffic_model.go      # NEW: other airline traffic
├── interline/                    # NEW: codeshare/interline
│   └── logic/
│       ├── agreement_logic.go
│       └── rebooking_rules.go
├── saga-flight-delay/
├── saga-crew-swap/
├── saga-passenger-rebook/
├── saga-cargo-reroute/
├── projector-network-state/
│   └── logic/
│       ├── network_projector.go
│       └── api_model.go          # NEW: API response types
├── projector-crew-legality/
├── projector-passenger-impact/
├── projector-cost-summary/       # NEW
├── dashboard/                    # NEW: web dashboard
│   ├── main.go
│   ├── api/
│   │   ├── routes.go
│   │   ├── flights.go
│   │   ├── airports.go
│   │   ├── crew.go
│   │   ├── passengers.go
│   │   ├── costs.go
│   │   └── simulation.go
│   ├── websocket/
│   │   ├── hub.go
│   │   └── handlers.go
│   └── static/                   # SPA frontend
│       ├── index.html
│       ├── js/
│       └── css/
├── simulation/                   # NEW: expanded
│   ├── clock.go
│   ├── scenario_runner.go
│   ├── disruption_injector.go
│   └── playback_controller.go
├── data/
│   ├── bts_loader.go
│   ├── airport_traffic_loader.go # NEW
│   ├── sample_schedule.json
│   └── README.md
└── features/
    ├── flight.feature
    ├── aircraft.feature
    ├── cockpit_crew.feature
    ├── cabin_crew.feature
    ├── passenger.feature
    ├── cargo.feature
    ├── airport_slot_contention.feature  # NEW
    ├── interline_rebooking.feature      # NEW
    ├── simulation_playback.feature      # NEW
    ├── saga_delay_propagation.feature
    └── saga_crew_swap.feature
```

---

## Additional Gherkin Scenarios

### Airport Slot Contention
```gherkin
Feature: Airport Slot Contention
  Tests that recovery solutions respect airport capacity.

  Scenario: Recovery blocked by competing traffic
    Given airport "JFK" has declared capacity of 60 arrivals/hour
    And the 14:00-15:00 slot has 58 arrivals from other airlines
    And our airline has 1 arrival scheduled at 14:30
    When we try to add a recovery flight arriving at 14:45
    Then the recovery is rejected
    And the reason is "airport capacity exceeded"

  Scenario: Recovery finds available slot
    Given airport "JFK" has declared capacity of 60 arrivals/hour
    And the 14:00-15:00 slot has 50 arrivals total
    And the 15:00-16:00 slot has 55 arrivals total
    When we optimize recovery for a diverted flight
    Then the system proposes arrival at 14:xx
    And the slot is marked as occupied
```

### Codeshare Handling
```gherkin
Feature: Codeshare Flight Disruption
  Tests handling of codeshare passengers during disruptions.

  Scenario: Rebook codeshare passenger on operating carrier
    Given flight "AA100" is operated by AA with codeshares BA4100, QF3100
    And passenger "PAX001" booked on "BA4100" (codeshare)
    When flight "AA100" is cancelled
    Then passenger "PAX001" can be rebooked on AA flights
    And the rebooking cost is charged to BA per interline agreement

  Scenario: Interline rebooking to partner airline
    Given passenger "PAX001" with itinerary AA100 -> connection -> BA200
    And AA and BA have an interline agreement
    When AA100 is delayed causing missed connection to BA200
    Then passenger can be rebooked on next BA flight
    And AA bears the rebooking cost
```

### Simulation Playback
```gherkin
Feature: Historical Scenario Playback
  Tests simulation of historical disruption events.

  Scenario: Replay JFK snowstorm closure
    Given historical data for 2024-01-15
    And JFK was closed 08:00-14:00 due to snow
    When I start simulation at 06:00 with speed 60x
    Then at simulation time 08:00 an AirportClosed event fires
    And affected flights are delayed or cancelled
    And at simulation time 14:00 an AirportReopened event fires
    And recovery sagas begin executing

  Scenario: Fast-forward to specific time
    Given simulation running at 10:00
    When I fast-forward to 14:00
    Then all events between 10:00-14:00 are processed
    And state reflects 14:00 conditions
```

---

## References

- [Integrated Airline Recovery (Transportation Science)](https://dl.acm.org/doi/abs/10.1287/trsc.1120.0414)
- [FAR Part 117 (eCFR)](https://www.ecfr.gov/current/title-14/chapter-I/subchapter-G/part-117)
- [BTS On-Time Data](https://www.bts.gov/browse-statistical-products-and-data/bts-publications/airline-service-quality-performance-234-time)
- [OAG: Loyalty Impact Survey 2023](https://www.oag.com/traveler-survey-2023)
- [IATA Delay Codes](https://grokipedia.com/page/IATA_delay_codes)
- [SKYbrary: MEL](https://skybrary.aero/articles/minimum-equipment-list-mel)
- [OAG: Turnaround Times](https://www.oag.com/blog/science-aircraft-turnarounds)
