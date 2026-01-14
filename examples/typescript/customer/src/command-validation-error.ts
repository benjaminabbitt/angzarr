/**
 * Error class for command validation failures.
 * Bridges domain validation to gRPC status codes without coupling logic to gRPC.
 */
export enum StatusCode {
  INVALID_ARGUMENT = 'INVALID_ARGUMENT',
  FAILED_PRECONDITION = 'FAILED_PRECONDITION',
}

export class CommandValidationError extends Error {
  constructor(
    message: string,
    public readonly statusCode: StatusCode
  ) {
    super(message);
    this.name = 'CommandValidationError';
  }

  static invalidArgument(message: string): CommandValidationError {
    return new CommandValidationError(message, StatusCode.INVALID_ARGUMENT);
  }

  static failedPrecondition(message: string): CommandValidationError {
    return new CommandValidationError(message, StatusCode.FAILED_PRECONDITION);
  }
}
