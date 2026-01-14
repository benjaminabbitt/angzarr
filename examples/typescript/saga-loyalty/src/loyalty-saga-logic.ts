/**
 * Pure business logic for loyalty saga.
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

export interface AddLoyaltyPointsCommand {
  domain: string;
  customerId: string;
  points: number;
  reason: string;
}

/**
 * Loyalty saga business logic.
 */
export class LoyaltySagaLogic {
  /**
   * Processes events and generates commands.
   */
  process(eventBook: EventBook | null | undefined): AddLoyaltyPointsCommand[] {
    const commands: AddLoyaltyPointsCommand[] = [];

    if (!eventBook?.pages?.length) {
      return commands;
    }

    const transactionId = this.uuidToHex(eventBook.cover?.root);

    for (const page of eventBook.pages) {
      const typeUrl = page.typeUrl || '';

      if (typeUrl.endsWith('TransactionCompleted')) {
        const points = page.data.loyaltyPointsEarned || 0;

        if (points > 0) {
          commands.push({
            domain: 'customer',
            customerId: this.uuidToHex(eventBook.cover?.root),
            points,
            reason: `transaction:${transactionId}`,
          });
        }
      }
    }

    return commands;
  }

  private uuidToHex(uuid: any): string {
    if (!uuid?.value) return '';
    return Buffer.from(uuid.value).toString('hex');
  }
}
