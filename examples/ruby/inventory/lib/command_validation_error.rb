# frozen_string_literal: true

# Error class for command validation failures.
# Bridges domain validation to gRPC status codes without coupling logic to gRPC.
class CommandValidationError < StandardError
  attr_reader :status_code

  INVALID_ARGUMENT = :invalid_argument
  FAILED_PRECONDITION = :failed_precondition

  def initialize(message, status_code)
    super(message)
    @status_code = status_code
  end

  def self.invalid_argument(message)
    new(message, INVALID_ARGUMENT)
  end

  def self.failed_precondition(message)
    new(message, FAILED_PRECONDITION)
  end
end
