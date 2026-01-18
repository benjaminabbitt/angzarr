# Aviation Network Recovery System

An event-sourced system for modeling and recovering an airline's flight network after disruptions (delays, cancellations, airport closures). Built on the Angzarr CQRS/Event Sourcing framework.

## Design Decisions

| Feature | Decision |
|---------|----------|
| Optimization Scope | Full network |
| Simulation Speed | Real-time + fast-forward (60x-3600x) |
| Multi-Airline | Codeshare/interline support |
| Scenarios | Historical data replay (BTS) |
| UI | Web dashboard with REST/WebSocket |

## Domain Overview

Airlines lose approximately $35 billion annually due to flight disruptions. This example demonstrates how event sourcing enables:

- **Full network optimization** - Consider all flights, crew, aircraft simultaneously
- **What-if analysis** - Replay scenarios with different recovery strategies
- **Downstream modeling** - Track cascading impacts through the network
- **Airport contention** - Evaluate slot availability against competing traffic
- **Codeshare handling** - Manage marketing vs operating carrier responsibilities

## Bounded Contexts (Aggregates)

| Aggregate | Description |
|-----------|-------------|
| `flight` | Flight lifecycle: scheduling, delays, cancellations, actual times |
| `aircraft` | Airframe tracking: position, maintenance status, MEL items |
| `cockpit-crew` | Pilots: duty hours, rest requirements, qualifications |
| `cabin-crew` | Flight attendants: duty hours, rest, positioning |
| `passenger` | Travelers: itineraries, loyalty tier, connections |
| `cargo` | Freight: SLAs, penalties, priority levels |
| `airport` | Resources: slots, gates, curfews, **competing traffic** |
| `interline` | Codeshare agreements, rebooking rules |

## Sagas (Process Managers)

| Saga | Purpose |
|------|---------|
| `saga-flight-delay` | Propagate delay impacts through network |
| `saga-crew-swap` | Reassign crew when duty limits exceeded |
| `saga-aircraft-swap` | Substitute aircraft when unavailable |
| `saga-passenger-rebook` | Find alternatives (including interline partners) |
| `saga-cargo-reroute` | Reroute freight to meet SLAs |

## Key Constraints Modeled

### Crew (FAR Part 117)
- 10-hour rest minimum before duty
- 8-9 hour max flight time (2-pilot crew)
- 30 consecutive hours rest per 168-hour period
- Qualification requirements per aircraft type

### Aircraft
- Minimum turnaround times (25-40 min narrow-body)
- MEL (Minimum Equipment List) repair categories
- Scheduled maintenance windows
- Position continuity

### Airport Slot Contention
- **All traffic modeled** - not just our airline
- Declared capacity limits (arrivals/departures per hour)
- Recovery proposals validated against available slots
- Curfews enforced (night operation restrictions)

### Codeshare/Interline
- Operating vs marketing carrier distinction
- Interline agreements define rebooking rules
- Cost allocation for disruption handling
- Partner airline capacity visibility

## Simulation Engine

### Playback Modes

| Mode | Speed | Use Case |
|------|-------|----------|
| Real-time | 1x | Training, live demo |
| Fast-forward | 60x-3600x | Scenario analysis |
| Step | Manual | Debugging, what-if |
| Instant | Max | Batch optimization |

### Historical Scenarios
Load actual disruption events from BTS data:
- Airport closures (weather, security)
- Flight delays with cause codes
- Competing airline traffic at airports

## Web Dashboard

### Views
1. **Network Map** - Geographic view, color-coded by status
2. **Crew Status** - Duty time remaining, position tracking
3. **Passenger Impact** - Disrupted passengers, rebooking status
4. **Cost Dashboard** - Running totals, penalty projections
5. **Simulation Control** - Play/pause/step, speed control

### API
```
REST:
  GET  /api/v1/flights
  GET  /api/v1/airports/{code}/traffic
  GET  /api/v1/crew/{id}/status
  GET  /api/v1/costs/summary
  POST /api/v1/simulation/speed
  POST /api/v1/simulation/inject

WebSocket:
  /ws/network    # Real-time network updates
  /ws/costs      # Cost ticker
  /ws/alerts     # Crew/connection alerts
```

## Data Sources

### BTS On-Time Performance
Historical US domestic flight data:
- https://www.transtats.bts.gov/Tables.asp?DB_ID=120
- All flights since 1987 with delay causes
- Used for both own-airline and competing traffic

### Airport Traffic
Load all carrier operations to model slot contention accurately.

## Commands

```bash
# Build
just build

# Run tests
just test

# Load historical scenario
just load-scenario --date 2024-01-15

# Start simulation (fast-forward)
just simulate --speed 60

# Start dashboard
just dashboard

# Inject disruption
just inject-delay --flight AA100 --minutes 90
just inject-closure --airport JFK --start 08:00 --end 14:00
```

## References

- [FAR Part 117: Flight/Duty Limits](https://www.ecfr.gov/current/title-14/chapter-I/subchapter-G/part-117)
- [BTS On-Time Data](https://www.bts.gov/topics/airline-time-tables)
- [IATA Delay Codes](https://grokipedia.com/page/IATA_delay_codes)
- [SKYbrary: MEL](https://skybrary.aero/articles/minimum-equipment-list-mel)
- [OAG: Turnaround Times](https://www.oag.com/blog/science-aircraft-turnarounds)
