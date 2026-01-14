/**
 * Pure business logic for customer log projector.
 * No gRPC dependencies - can be tested in isolation.
 */

export interface EventPage {
  typeUrl: string;
  data: any;
}

export interface EventBook {
  cover?: {
    domain?: string;
    root?: { value?: Uint8Array };
  };
  pages: EventPage[];
}

export interface LogEntry {
  domain: string;
  rootId: string;
  sequence: number;
  eventType: string;
  fields: Record<string, any>;
}

/**
 * Customer log projector business logic.
 */
export class LogProjectorLogic {
  /**
   * Processes an event book and returns log entries.
   */
  processEvents(eventBook: EventBook | null | undefined): LogEntry[] {
    const entries: LogEntry[] = [];

    if (!eventBook?.pages?.length) {
      return entries;
    }

    const domain = eventBook.cover?.domain || 'customer';
    const rootId = this.uuidToHex(eventBook.cover?.root);
    const shortId = rootId.slice(0, 16);

    for (let i = 0; i < eventBook.pages.length; i++) {
      const page = eventBook.pages[i];
      const typeUrl = page.typeUrl || '';
      const eventType = typeUrl.split('.').pop() || typeUrl;
      const sequence = i;

      const entry: LogEntry = {
        domain,
        rootId: shortId,
        sequence,
        eventType,
        fields: {},
      };

      if (typeUrl.endsWith('CustomerCreated')) {
        entry.fields = {
          name: page.data.name,
          email: page.data.email,
        };
      } else if (typeUrl.endsWith('LoyaltyPointsAdded')) {
        entry.fields = {
          points: page.data.points,
          newBalance: page.data.newBalance,
          reason: page.data.reason,
        };
      } else if (typeUrl.endsWith('LoyaltyPointsRedeemed')) {
        entry.fields = {
          points: page.data.points,
          newBalance: page.data.newBalance,
          redemptionType: page.data.redemptionType,
        };
      } else {
        entry.fields = { unknown: true };
      }

      entries.push(entry);
    }

    return entries;
  }

  private uuidToHex(uuid: any): string {
    if (!uuid?.value) return '';
    return Buffer.from(uuid.value).toString('hex');
  }
}
