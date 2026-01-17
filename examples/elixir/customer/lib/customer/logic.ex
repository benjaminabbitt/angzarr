defmodule Customer.Logic do
  @moduledoc """
  Pure business logic for customer aggregate.
  No gRPC dependencies - can be tested in isolation.
  """

  defmodule State do
    defstruct name: "", email: "", loyalty_points: 0, lifetime_points: 0

    def exists?(%State{name: name}), do: name != ""
  end

  def rebuild_state(nil), do: %State{}
  def rebuild_state(%{pages: []}), do: %State{}

  def rebuild_state(event_book) do
    initial_state =
      case event_book.snapshot do
        %{state: %{type_url: type_url, value: value}} when type_url != nil ->
          if String.ends_with?(type_url, "CustomerState") do
            snap = Examples.CustomerState.decode(value)
            %State{
              name: snap.name,
              email: snap.email,
              loyalty_points: snap.loyalty_points,
              lifetime_points: snap.lifetime_points
            }
          else
            %State{}
          end
        _ ->
          %State{}
      end

    Enum.reduce(event_book.pages, initial_state, fn page, state ->
      if page.event, do: apply_event(state, page.event), else: state
    end)
  end

  def handle_create_customer(state, name, email) do
    cond do
      State.exists?(state) ->
        {:error, :failed_precondition, "Customer already exists"}

      is_nil(name) or name == "" ->
        {:error, :invalid_argument, "Customer name is required"}

      is_nil(email) or email == "" ->
        {:error, :invalid_argument, "Customer email is required"}

      true ->
        {:ok, Examples.CustomerCreated.new(
          name: name,
          email: email,
          created_at: now_timestamp()
        )}
    end
  end

  def handle_add_loyalty_points(state, points, reason) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Customer does not exist"}

      points <= 0 ->
        {:error, :invalid_argument, "Points must be positive"}

      true ->
        new_balance = state.loyalty_points + points
        {:ok, Examples.LoyaltyPointsAdded.new(
          points: points,
          new_balance: new_balance,
          reason: reason || ""
        )}
    end
  end

  def handle_redeem_loyalty_points(state, points, redemption_type) do
    cond do
      not State.exists?(state) ->
        {:error, :failed_precondition, "Customer does not exist"}

      points <= 0 ->
        {:error, :invalid_argument, "Points must be positive"}

      points > state.loyalty_points ->
        {:error, :failed_precondition, "Insufficient points: have #{state.loyalty_points}, need #{points}"}

      true ->
        new_balance = state.loyalty_points - points
        {:ok, Examples.LoyaltyPointsRedeemed.new(
          points: points,
          new_balance: new_balance,
          redemption_type: redemption_type || ""
        )}
    end
  end

  defp apply_event(state, event_any) do
    type_url = event_any.type_url

    cond do
      String.ends_with?(type_url, "CustomerCreated") ->
        event = Examples.CustomerCreated.decode(event_any.value)
        %State{state | name: event.name, email: event.email}

      String.ends_with?(type_url, "LoyaltyPointsAdded") ->
        event = Examples.LoyaltyPointsAdded.decode(event_any.value)
        %State{state |
          loyalty_points: event.new_balance,
          lifetime_points: state.lifetime_points + event.points
        }

      String.ends_with?(type_url, "LoyaltyPointsRedeemed") ->
        event = Examples.LoyaltyPointsRedeemed.decode(event_any.value)
        %State{state | loyalty_points: event.new_balance}

      true ->
        state
    end
  end

  defp now_timestamp do
    Google.Protobuf.Timestamp.new(seconds: System.system_time(:second))
  end
end
