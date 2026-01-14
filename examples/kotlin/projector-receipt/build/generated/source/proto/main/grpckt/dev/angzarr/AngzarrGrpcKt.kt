package dev.angzarr

import com.google.protobuf.Empty
import io.grpc.CallOptions
import io.grpc.CallOptions.DEFAULT
import io.grpc.Channel
import io.grpc.Metadata
import io.grpc.MethodDescriptor
import io.grpc.ServerServiceDefinition
import io.grpc.ServerServiceDefinition.builder
import io.grpc.ServiceDescriptor
import io.grpc.Status.UNIMPLEMENTED
import io.grpc.StatusException
import io.grpc.kotlin.AbstractCoroutineServerImpl
import io.grpc.kotlin.AbstractCoroutineStub
import io.grpc.kotlin.ClientCalls.bidiStreamingRpc
import io.grpc.kotlin.ClientCalls.serverStreamingRpc
import io.grpc.kotlin.ClientCalls.unaryRpc
import io.grpc.kotlin.ServerCalls.bidiStreamingServerMethodDefinition
import io.grpc.kotlin.ServerCalls.serverStreamingServerMethodDefinition
import io.grpc.kotlin.ServerCalls.unaryServerMethodDefinition
import io.grpc.kotlin.StubFor
import kotlin.String
import kotlin.coroutines.CoroutineContext
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.jvm.JvmOverloads
import kotlin.jvm.JvmStatic
import kotlinx.coroutines.flow.Flow
import dev.angzarr.BusinessCoordinatorGrpc.getServiceDescriptor as businessCoordinatorGrpcGetServiceDescriptor
import dev.angzarr.BusinessLogicGrpc.getServiceDescriptor as businessLogicGrpcGetServiceDescriptor
import dev.angzarr.CommandProxyGrpc.getServiceDescriptor as commandProxyGrpcGetServiceDescriptor
import dev.angzarr.EventQueryGrpc.getServiceDescriptor as eventQueryGrpcGetServiceDescriptor
import dev.angzarr.EventStreamGrpc.getServiceDescriptor as eventStreamGrpcGetServiceDescriptor
import dev.angzarr.ProjectorCoordinatorGrpc.getServiceDescriptor as projectorCoordinatorGrpcGetServiceDescriptor
import dev.angzarr.ProjectorGrpc.getServiceDescriptor as projectorGrpcGetServiceDescriptor
import dev.angzarr.SagaCoordinatorGrpc.getServiceDescriptor as sagaCoordinatorGrpcGetServiceDescriptor
import dev.angzarr.SagaGrpc.getServiceDescriptor as sagaGrpcGetServiceDescriptor

/**
 * Holder for Kotlin coroutine-based client and server APIs for angzarr.BusinessLogic.
 */
public object BusinessLogicGrpcKt {
  public const val SERVICE_NAME: String = BusinessLogicGrpc.SERVICE_NAME

  @JvmStatic
  public val serviceDescriptor: ServiceDescriptor
    get() = businessLogicGrpcGetServiceDescriptor()

  public val handleMethod: MethodDescriptor<ContextualCommand, BusinessResponse>
    @JvmStatic
    get() = BusinessLogicGrpc.getHandleMethod()

  /**
   * A stub for issuing RPCs to a(n) angzarr.BusinessLogic service as suspending coroutines.
   */
  @StubFor(BusinessLogicGrpc::class)
  public class BusinessLogicCoroutineStub @JvmOverloads constructor(
    channel: Channel,
    callOptions: CallOptions = DEFAULT,
  ) : AbstractCoroutineStub<BusinessLogicCoroutineStub>(channel, callOptions) {
    override fun build(channel: Channel, callOptions: CallOptions): BusinessLogicCoroutineStub =
        BusinessLogicCoroutineStub(channel, callOptions)

    /**
     * Executes this RPC and returns the response message, suspending until the RPC completes
     * with [`Status.OK`][io.grpc.Status].  If the RPC completes with another status, a
     * corresponding
     * [StatusException] is thrown.  If this coroutine is cancelled, the RPC is also cancelled
     * with the corresponding exception as a cause.
     *
     * @param request The request message to send to the server.
     *
     * @param headers Metadata to attach to the request.  Most users will not need this.
     *
     * @return The single response from the server.
     */
    public suspend fun handle(request: ContextualCommand, headers: Metadata = Metadata()):
        BusinessResponse = unaryRpc(
      channel,
      BusinessLogicGrpc.getHandleMethod(),
      request,
      callOptions,
      headers
    )
  }

  /**
   * Skeletal implementation of the angzarr.BusinessLogic service based on Kotlin coroutines.
   */
  public abstract class BusinessLogicCoroutineImplBase(
    coroutineContext: CoroutineContext = EmptyCoroutineContext,
  ) : AbstractCoroutineServerImpl(coroutineContext) {
    /**
     * Returns the response to an RPC for angzarr.BusinessLogic.Handle.
     *
     * If this method fails with a [StatusException], the RPC will fail with the corresponding
     * [io.grpc.Status].  If this method fails with a [java.util.concurrent.CancellationException],
     * the RPC will fail
     * with status `Status.CANCELLED`.  If this method fails for any other reason, the RPC will
     * fail with `Status.UNKNOWN` with the exception as a cause.
     *
     * @param request The request from the client.
     */
    public open suspend fun handle(request: ContextualCommand): BusinessResponse = throw
        StatusException(UNIMPLEMENTED.withDescription("Method angzarr.BusinessLogic.Handle is unimplemented"))

    final override fun bindService(): ServerServiceDefinition =
        builder(businessLogicGrpcGetServiceDescriptor())
      .addMethod(unaryServerMethodDefinition(
      context = this.context,
      descriptor = BusinessLogicGrpc.getHandleMethod(),
      implementation = ::handle
    )).build()
  }
}

/**
 * Holder for Kotlin coroutine-based client and server APIs for angzarr.BusinessCoordinator.
 */
public object BusinessCoordinatorGrpcKt {
  public const val SERVICE_NAME: String = BusinessCoordinatorGrpc.SERVICE_NAME

  @JvmStatic
  public val serviceDescriptor: ServiceDescriptor
    get() = businessCoordinatorGrpcGetServiceDescriptor()

  public val handleMethod: MethodDescriptor<CommandBook, CommandResponse>
    @JvmStatic
    get() = BusinessCoordinatorGrpc.getHandleMethod()

  public val recordMethod: MethodDescriptor<EventBook, CommandResponse>
    @JvmStatic
    get() = BusinessCoordinatorGrpc.getRecordMethod()

  /**
   * A stub for issuing RPCs to a(n) angzarr.BusinessCoordinator service as suspending coroutines.
   */
  @StubFor(BusinessCoordinatorGrpc::class)
  public class BusinessCoordinatorCoroutineStub @JvmOverloads constructor(
    channel: Channel,
    callOptions: CallOptions = DEFAULT,
  ) : AbstractCoroutineStub<BusinessCoordinatorCoroutineStub>(channel, callOptions) {
    override fun build(channel: Channel, callOptions: CallOptions): BusinessCoordinatorCoroutineStub
        = BusinessCoordinatorCoroutineStub(channel, callOptions)

    /**
     * Executes this RPC and returns the response message, suspending until the RPC completes
     * with [`Status.OK`][io.grpc.Status].  If the RPC completes with another status, a
     * corresponding
     * [StatusException] is thrown.  If this coroutine is cancelled, the RPC is also cancelled
     * with the corresponding exception as a cause.
     *
     * @param request The request message to send to the server.
     *
     * @param headers Metadata to attach to the request.  Most users will not need this.
     *
     * @return The single response from the server.
     */
    public suspend fun handle(request: CommandBook, headers: Metadata = Metadata()): CommandResponse
        = unaryRpc(
      channel,
      BusinessCoordinatorGrpc.getHandleMethod(),
      request,
      callOptions,
      headers
    )

    /**
     * Executes this RPC and returns the response message, suspending until the RPC completes
     * with [`Status.OK`][io.grpc.Status].  If the RPC completes with another status, a
     * corresponding
     * [StatusException] is thrown.  If this coroutine is cancelled, the RPC is also cancelled
     * with the corresponding exception as a cause.
     *
     * @param request The request message to send to the server.
     *
     * @param headers Metadata to attach to the request.  Most users will not need this.
     *
     * @return The single response from the server.
     */
    public suspend fun record(request: EventBook, headers: Metadata = Metadata()): CommandResponse =
        unaryRpc(
      channel,
      BusinessCoordinatorGrpc.getRecordMethod(),
      request,
      callOptions,
      headers
    )
  }

  /**
   * Skeletal implementation of the angzarr.BusinessCoordinator service based on Kotlin coroutines.
   */
  public abstract class BusinessCoordinatorCoroutineImplBase(
    coroutineContext: CoroutineContext = EmptyCoroutineContext,
  ) : AbstractCoroutineServerImpl(coroutineContext) {
    /**
     * Returns the response to an RPC for angzarr.BusinessCoordinator.Handle.
     *
     * If this method fails with a [StatusException], the RPC will fail with the corresponding
     * [io.grpc.Status].  If this method fails with a [java.util.concurrent.CancellationException],
     * the RPC will fail
     * with status `Status.CANCELLED`.  If this method fails for any other reason, the RPC will
     * fail with `Status.UNKNOWN` with the exception as a cause.
     *
     * @param request The request from the client.
     */
    public open suspend fun handle(request: CommandBook): CommandResponse = throw
        StatusException(UNIMPLEMENTED.withDescription("Method angzarr.BusinessCoordinator.Handle is unimplemented"))

    /**
     * Returns the response to an RPC for angzarr.BusinessCoordinator.Record.
     *
     * If this method fails with a [StatusException], the RPC will fail with the corresponding
     * [io.grpc.Status].  If this method fails with a [java.util.concurrent.CancellationException],
     * the RPC will fail
     * with status `Status.CANCELLED`.  If this method fails for any other reason, the RPC will
     * fail with `Status.UNKNOWN` with the exception as a cause.
     *
     * @param request The request from the client.
     */
    public open suspend fun record(request: EventBook): CommandResponse = throw
        StatusException(UNIMPLEMENTED.withDescription("Method angzarr.BusinessCoordinator.Record is unimplemented"))

    final override fun bindService(): ServerServiceDefinition =
        builder(businessCoordinatorGrpcGetServiceDescriptor())
      .addMethod(unaryServerMethodDefinition(
      context = this.context,
      descriptor = BusinessCoordinatorGrpc.getHandleMethod(),
      implementation = ::handle
    ))
      .addMethod(unaryServerMethodDefinition(
      context = this.context,
      descriptor = BusinessCoordinatorGrpc.getRecordMethod(),
      implementation = ::record
    )).build()
  }
}

/**
 * Holder for Kotlin coroutine-based client and server APIs for angzarr.ProjectorCoordinator.
 */
public object ProjectorCoordinatorGrpcKt {
  public const val SERVICE_NAME: String = ProjectorCoordinatorGrpc.SERVICE_NAME

  @JvmStatic
  public val serviceDescriptor: ServiceDescriptor
    get() = projectorCoordinatorGrpcGetServiceDescriptor()

  public val handleSyncMethod: MethodDescriptor<EventBook, Projection>
    @JvmStatic
    get() = ProjectorCoordinatorGrpc.getHandleSyncMethod()

  public val handleMethod: MethodDescriptor<EventBook, Empty>
    @JvmStatic
    get() = ProjectorCoordinatorGrpc.getHandleMethod()

  /**
   * A stub for issuing RPCs to a(n) angzarr.ProjectorCoordinator service as suspending coroutines.
   */
  @StubFor(ProjectorCoordinatorGrpc::class)
  public class ProjectorCoordinatorCoroutineStub @JvmOverloads constructor(
    channel: Channel,
    callOptions: CallOptions = DEFAULT,
  ) : AbstractCoroutineStub<ProjectorCoordinatorCoroutineStub>(channel, callOptions) {
    override fun build(channel: Channel, callOptions: CallOptions):
        ProjectorCoordinatorCoroutineStub = ProjectorCoordinatorCoroutineStub(channel, callOptions)

    /**
     * Executes this RPC and returns the response message, suspending until the RPC completes
     * with [`Status.OK`][io.grpc.Status].  If the RPC completes with another status, a
     * corresponding
     * [StatusException] is thrown.  If this coroutine is cancelled, the RPC is also cancelled
     * with the corresponding exception as a cause.
     *
     * @param request The request message to send to the server.
     *
     * @param headers Metadata to attach to the request.  Most users will not need this.
     *
     * @return The single response from the server.
     */
    public suspend fun handleSync(request: EventBook, headers: Metadata = Metadata()): Projection =
        unaryRpc(
      channel,
      ProjectorCoordinatorGrpc.getHandleSyncMethod(),
      request,
      callOptions,
      headers
    )

    /**
     * Executes this RPC and returns the response message, suspending until the RPC completes
     * with [`Status.OK`][io.grpc.Status].  If the RPC completes with another status, a
     * corresponding
     * [StatusException] is thrown.  If this coroutine is cancelled, the RPC is also cancelled
     * with the corresponding exception as a cause.
     *
     * @param request The request message to send to the server.
     *
     * @param headers Metadata to attach to the request.  Most users will not need this.
     *
     * @return The single response from the server.
     */
    public suspend fun handle(request: EventBook, headers: Metadata = Metadata()): Empty = unaryRpc(
      channel,
      ProjectorCoordinatorGrpc.getHandleMethod(),
      request,
      callOptions,
      headers
    )
  }

  /**
   * Skeletal implementation of the angzarr.ProjectorCoordinator service based on Kotlin coroutines.
   */
  public abstract class ProjectorCoordinatorCoroutineImplBase(
    coroutineContext: CoroutineContext = EmptyCoroutineContext,
  ) : AbstractCoroutineServerImpl(coroutineContext) {
    /**
     * Returns the response to an RPC for angzarr.ProjectorCoordinator.HandleSync.
     *
     * If this method fails with a [StatusException], the RPC will fail with the corresponding
     * [io.grpc.Status].  If this method fails with a [java.util.concurrent.CancellationException],
     * the RPC will fail
     * with status `Status.CANCELLED`.  If this method fails for any other reason, the RPC will
     * fail with `Status.UNKNOWN` with the exception as a cause.
     *
     * @param request The request from the client.
     */
    public open suspend fun handleSync(request: EventBook): Projection = throw
        StatusException(UNIMPLEMENTED.withDescription("Method angzarr.ProjectorCoordinator.HandleSync is unimplemented"))

    /**
     * Returns the response to an RPC for angzarr.ProjectorCoordinator.Handle.
     *
     * If this method fails with a [StatusException], the RPC will fail with the corresponding
     * [io.grpc.Status].  If this method fails with a [java.util.concurrent.CancellationException],
     * the RPC will fail
     * with status `Status.CANCELLED`.  If this method fails for any other reason, the RPC will
     * fail with `Status.UNKNOWN` with the exception as a cause.
     *
     * @param request The request from the client.
     */
    public open suspend fun handle(request: EventBook): Empty = throw
        StatusException(UNIMPLEMENTED.withDescription("Method angzarr.ProjectorCoordinator.Handle is unimplemented"))

    final override fun bindService(): ServerServiceDefinition =
        builder(projectorCoordinatorGrpcGetServiceDescriptor())
      .addMethod(unaryServerMethodDefinition(
      context = this.context,
      descriptor = ProjectorCoordinatorGrpc.getHandleSyncMethod(),
      implementation = ::handleSync
    ))
      .addMethod(unaryServerMethodDefinition(
      context = this.context,
      descriptor = ProjectorCoordinatorGrpc.getHandleMethod(),
      implementation = ::handle
    )).build()
  }
}

/**
 * Holder for Kotlin coroutine-based client and server APIs for angzarr.Projector.
 */
public object ProjectorGrpcKt {
  public const val SERVICE_NAME: String = ProjectorGrpc.SERVICE_NAME

  @JvmStatic
  public val serviceDescriptor: ServiceDescriptor
    get() = projectorGrpcGetServiceDescriptor()

  public val handleMethod: MethodDescriptor<EventBook, Empty>
    @JvmStatic
    get() = ProjectorGrpc.getHandleMethod()

  public val handleSyncMethod: MethodDescriptor<EventBook, Projection>
    @JvmStatic
    get() = ProjectorGrpc.getHandleSyncMethod()

  /**
   * A stub for issuing RPCs to a(n) angzarr.Projector service as suspending coroutines.
   */
  @StubFor(ProjectorGrpc::class)
  public class ProjectorCoroutineStub @JvmOverloads constructor(
    channel: Channel,
    callOptions: CallOptions = DEFAULT,
  ) : AbstractCoroutineStub<ProjectorCoroutineStub>(channel, callOptions) {
    override fun build(channel: Channel, callOptions: CallOptions): ProjectorCoroutineStub =
        ProjectorCoroutineStub(channel, callOptions)

    /**
     * Executes this RPC and returns the response message, suspending until the RPC completes
     * with [`Status.OK`][io.grpc.Status].  If the RPC completes with another status, a
     * corresponding
     * [StatusException] is thrown.  If this coroutine is cancelled, the RPC is also cancelled
     * with the corresponding exception as a cause.
     *
     * @param request The request message to send to the server.
     *
     * @param headers Metadata to attach to the request.  Most users will not need this.
     *
     * @return The single response from the server.
     */
    public suspend fun handle(request: EventBook, headers: Metadata = Metadata()): Empty = unaryRpc(
      channel,
      ProjectorGrpc.getHandleMethod(),
      request,
      callOptions,
      headers
    )

    /**
     * Executes this RPC and returns the response message, suspending until the RPC completes
     * with [`Status.OK`][io.grpc.Status].  If the RPC completes with another status, a
     * corresponding
     * [StatusException] is thrown.  If this coroutine is cancelled, the RPC is also cancelled
     * with the corresponding exception as a cause.
     *
     * @param request The request message to send to the server.
     *
     * @param headers Metadata to attach to the request.  Most users will not need this.
     *
     * @return The single response from the server.
     */
    public suspend fun handleSync(request: EventBook, headers: Metadata = Metadata()): Projection =
        unaryRpc(
      channel,
      ProjectorGrpc.getHandleSyncMethod(),
      request,
      callOptions,
      headers
    )
  }

  /**
   * Skeletal implementation of the angzarr.Projector service based on Kotlin coroutines.
   */
  public abstract class ProjectorCoroutineImplBase(
    coroutineContext: CoroutineContext = EmptyCoroutineContext,
  ) : AbstractCoroutineServerImpl(coroutineContext) {
    /**
     * Returns the response to an RPC for angzarr.Projector.Handle.
     *
     * If this method fails with a [StatusException], the RPC will fail with the corresponding
     * [io.grpc.Status].  If this method fails with a [java.util.concurrent.CancellationException],
     * the RPC will fail
     * with status `Status.CANCELLED`.  If this method fails for any other reason, the RPC will
     * fail with `Status.UNKNOWN` with the exception as a cause.
     *
     * @param request The request from the client.
     */
    public open suspend fun handle(request: EventBook): Empty = throw
        StatusException(UNIMPLEMENTED.withDescription("Method angzarr.Projector.Handle is unimplemented"))

    /**
     * Returns the response to an RPC for angzarr.Projector.HandleSync.
     *
     * If this method fails with a [StatusException], the RPC will fail with the corresponding
     * [io.grpc.Status].  If this method fails with a [java.util.concurrent.CancellationException],
     * the RPC will fail
     * with status `Status.CANCELLED`.  If this method fails for any other reason, the RPC will
     * fail with `Status.UNKNOWN` with the exception as a cause.
     *
     * @param request The request from the client.
     */
    public open suspend fun handleSync(request: EventBook): Projection = throw
        StatusException(UNIMPLEMENTED.withDescription("Method angzarr.Projector.HandleSync is unimplemented"))

    final override fun bindService(): ServerServiceDefinition =
        builder(projectorGrpcGetServiceDescriptor())
      .addMethod(unaryServerMethodDefinition(
      context = this.context,
      descriptor = ProjectorGrpc.getHandleMethod(),
      implementation = ::handle
    ))
      .addMethod(unaryServerMethodDefinition(
      context = this.context,
      descriptor = ProjectorGrpc.getHandleSyncMethod(),
      implementation = ::handleSync
    )).build()
  }
}

/**
 * Holder for Kotlin coroutine-based client and server APIs for angzarr.EventQuery.
 */
public object EventQueryGrpcKt {
  public const val SERVICE_NAME: String = EventQueryGrpc.SERVICE_NAME

  @JvmStatic
  public val serviceDescriptor: ServiceDescriptor
    get() = eventQueryGrpcGetServiceDescriptor()

  public val getEventBookMethod: MethodDescriptor<Query, EventBook>
    @JvmStatic
    get() = EventQueryGrpc.getGetEventBookMethod()

  public val getEventsMethod: MethodDescriptor<Query, EventBook>
    @JvmStatic
    get() = EventQueryGrpc.getGetEventsMethod()

  public val synchronizeMethod: MethodDescriptor<Query, EventBook>
    @JvmStatic
    get() = EventQueryGrpc.getSynchronizeMethod()

  public val getAggregateRootsMethod: MethodDescriptor<Empty, AggregateRoot>
    @JvmStatic
    get() = EventQueryGrpc.getGetAggregateRootsMethod()

  /**
   * A stub for issuing RPCs to a(n) angzarr.EventQuery service as suspending coroutines.
   */
  @StubFor(EventQueryGrpc::class)
  public class EventQueryCoroutineStub @JvmOverloads constructor(
    channel: Channel,
    callOptions: CallOptions = DEFAULT,
  ) : AbstractCoroutineStub<EventQueryCoroutineStub>(channel, callOptions) {
    override fun build(channel: Channel, callOptions: CallOptions): EventQueryCoroutineStub =
        EventQueryCoroutineStub(channel, callOptions)

    /**
     * Executes this RPC and returns the response message, suspending until the RPC completes
     * with [`Status.OK`][io.grpc.Status].  If the RPC completes with another status, a
     * corresponding
     * [StatusException] is thrown.  If this coroutine is cancelled, the RPC is also cancelled
     * with the corresponding exception as a cause.
     *
     * @param request The request message to send to the server.
     *
     * @param headers Metadata to attach to the request.  Most users will not need this.
     *
     * @return The single response from the server.
     */
    public suspend fun getEventBook(request: Query, headers: Metadata = Metadata()): EventBook =
        unaryRpc(
      channel,
      EventQueryGrpc.getGetEventBookMethod(),
      request,
      callOptions,
      headers
    )

    /**
     * Returns a [Flow] that, when collected, executes this RPC and emits responses from the
     * server as they arrive.  That flow finishes normally if the server closes its response with
     * [`Status.OK`][io.grpc.Status], and fails by throwing a [StatusException] otherwise.  If
     * collecting the flow downstream fails exceptionally (including via cancellation), the RPC
     * is cancelled with that exception as a cause.
     *
     * @param request The request message to send to the server.
     *
     * @param headers Metadata to attach to the request.  Most users will not need this.
     *
     * @return A flow that, when collected, emits the responses from the server.
     */
    public fun getEvents(request: Query, headers: Metadata = Metadata()): Flow<EventBook> =
        serverStreamingRpc(
      channel,
      EventQueryGrpc.getGetEventsMethod(),
      request,
      callOptions,
      headers
    )

    /**
     * Returns a [Flow] that, when collected, executes this RPC and emits responses from the
     * server as they arrive.  That flow finishes normally if the server closes its response with
     * [`Status.OK`][io.grpc.Status], and fails by throwing a [StatusException] otherwise.  If
     * collecting the flow downstream fails exceptionally (including via cancellation), the RPC
     * is cancelled with that exception as a cause.
     *
     * The [Flow] of requests is collected once each time the [Flow] of responses is
     * collected. If collection of the [Flow] of responses completes normally or
     * exceptionally before collection of `requests` completes, the collection of
     * `requests` is cancelled.  If the collection of `requests` completes
     * exceptionally for any other reason, then the collection of the [Flow] of responses
     * completes exceptionally for the same reason and the RPC is cancelled with that reason.
     *
     * @param requests A [Flow] of request messages.
     *
     * @param headers Metadata to attach to the request.  Most users will not need this.
     *
     * @return A flow that, when collected, emits the responses from the server.
     */
    public fun synchronize(requests: Flow<Query>, headers: Metadata = Metadata()): Flow<EventBook> =
        bidiStreamingRpc(
      channel,
      EventQueryGrpc.getSynchronizeMethod(),
      requests,
      callOptions,
      headers
    )

    /**
     * Returns a [Flow] that, when collected, executes this RPC and emits responses from the
     * server as they arrive.  That flow finishes normally if the server closes its response with
     * [`Status.OK`][io.grpc.Status], and fails by throwing a [StatusException] otherwise.  If
     * collecting the flow downstream fails exceptionally (including via cancellation), the RPC
     * is cancelled with that exception as a cause.
     *
     * @param request The request message to send to the server.
     *
     * @param headers Metadata to attach to the request.  Most users will not need this.
     *
     * @return A flow that, when collected, emits the responses from the server.
     */
    public fun getAggregateRoots(request: Empty, headers: Metadata = Metadata()):
        Flow<AggregateRoot> = serverStreamingRpc(
      channel,
      EventQueryGrpc.getGetAggregateRootsMethod(),
      request,
      callOptions,
      headers
    )
  }

  /**
   * Skeletal implementation of the angzarr.EventQuery service based on Kotlin coroutines.
   */
  public abstract class EventQueryCoroutineImplBase(
    coroutineContext: CoroutineContext = EmptyCoroutineContext,
  ) : AbstractCoroutineServerImpl(coroutineContext) {
    /**
     * Returns the response to an RPC for angzarr.EventQuery.GetEventBook.
     *
     * If this method fails with a [StatusException], the RPC will fail with the corresponding
     * [io.grpc.Status].  If this method fails with a [java.util.concurrent.CancellationException],
     * the RPC will fail
     * with status `Status.CANCELLED`.  If this method fails for any other reason, the RPC will
     * fail with `Status.UNKNOWN` with the exception as a cause.
     *
     * @param request The request from the client.
     */
    public open suspend fun getEventBook(request: Query): EventBook = throw
        StatusException(UNIMPLEMENTED.withDescription("Method angzarr.EventQuery.GetEventBook is unimplemented"))

    /**
     * Returns a [Flow] of responses to an RPC for angzarr.EventQuery.GetEvents.
     *
     * If creating or collecting the returned flow fails with a [StatusException], the RPC
     * will fail with the corresponding [io.grpc.Status].  If it fails with a
     * [java.util.concurrent.CancellationException], the RPC will fail with status
     * `Status.CANCELLED`.  If creating
     * or collecting the returned flow fails for any other reason, the RPC will fail with
     * `Status.UNKNOWN` with the exception as a cause.
     *
     * @param request The request from the client.
     */
    public open fun getEvents(request: Query): Flow<EventBook> = throw
        StatusException(UNIMPLEMENTED.withDescription("Method angzarr.EventQuery.GetEvents is unimplemented"))

    /**
     * Returns a [Flow] of responses to an RPC for angzarr.EventQuery.Synchronize.
     *
     * If creating or collecting the returned flow fails with a [StatusException], the RPC
     * will fail with the corresponding [io.grpc.Status].  If it fails with a
     * [java.util.concurrent.CancellationException], the RPC will fail with status
     * `Status.CANCELLED`.  If creating
     * or collecting the returned flow fails for any other reason, the RPC will fail with
     * `Status.UNKNOWN` with the exception as a cause.
     *
     * @param requests A [Flow] of requests from the client.  This flow can be
     *        collected only once and throws [java.lang.IllegalStateException] on attempts to
     * collect
     *        it more than once.
     */
    public open fun synchronize(requests: Flow<Query>): Flow<EventBook> = throw
        StatusException(UNIMPLEMENTED.withDescription("Method angzarr.EventQuery.Synchronize is unimplemented"))

    /**
     * Returns a [Flow] of responses to an RPC for angzarr.EventQuery.GetAggregateRoots.
     *
     * If creating or collecting the returned flow fails with a [StatusException], the RPC
     * will fail with the corresponding [io.grpc.Status].  If it fails with a
     * [java.util.concurrent.CancellationException], the RPC will fail with status
     * `Status.CANCELLED`.  If creating
     * or collecting the returned flow fails for any other reason, the RPC will fail with
     * `Status.UNKNOWN` with the exception as a cause.
     *
     * @param request The request from the client.
     */
    public open fun getAggregateRoots(request: Empty): Flow<AggregateRoot> = throw
        StatusException(UNIMPLEMENTED.withDescription("Method angzarr.EventQuery.GetAggregateRoots is unimplemented"))

    final override fun bindService(): ServerServiceDefinition =
        builder(eventQueryGrpcGetServiceDescriptor())
      .addMethod(unaryServerMethodDefinition(
      context = this.context,
      descriptor = EventQueryGrpc.getGetEventBookMethod(),
      implementation = ::getEventBook
    ))
      .addMethod(serverStreamingServerMethodDefinition(
      context = this.context,
      descriptor = EventQueryGrpc.getGetEventsMethod(),
      implementation = ::getEvents
    ))
      .addMethod(bidiStreamingServerMethodDefinition(
      context = this.context,
      descriptor = EventQueryGrpc.getSynchronizeMethod(),
      implementation = ::synchronize
    ))
      .addMethod(serverStreamingServerMethodDefinition(
      context = this.context,
      descriptor = EventQueryGrpc.getGetAggregateRootsMethod(),
      implementation = ::getAggregateRoots
    )).build()
  }
}

/**
 * Holder for Kotlin coroutine-based client and server APIs for angzarr.SagaCoordinator.
 */
public object SagaCoordinatorGrpcKt {
  public const val SERVICE_NAME: String = SagaCoordinatorGrpc.SERVICE_NAME

  @JvmStatic
  public val serviceDescriptor: ServiceDescriptor
    get() = sagaCoordinatorGrpcGetServiceDescriptor()

  public val handleMethod: MethodDescriptor<EventBook, Empty>
    @JvmStatic
    get() = SagaCoordinatorGrpc.getHandleMethod()

  public val handleSyncMethod: MethodDescriptor<EventBook, SagaResponse>
    @JvmStatic
    get() = SagaCoordinatorGrpc.getHandleSyncMethod()

  /**
   * A stub for issuing RPCs to a(n) angzarr.SagaCoordinator service as suspending coroutines.
   */
  @StubFor(SagaCoordinatorGrpc::class)
  public class SagaCoordinatorCoroutineStub @JvmOverloads constructor(
    channel: Channel,
    callOptions: CallOptions = DEFAULT,
  ) : AbstractCoroutineStub<SagaCoordinatorCoroutineStub>(channel, callOptions) {
    override fun build(channel: Channel, callOptions: CallOptions): SagaCoordinatorCoroutineStub =
        SagaCoordinatorCoroutineStub(channel, callOptions)

    /**
     * Executes this RPC and returns the response message, suspending until the RPC completes
     * with [`Status.OK`][io.grpc.Status].  If the RPC completes with another status, a
     * corresponding
     * [StatusException] is thrown.  If this coroutine is cancelled, the RPC is also cancelled
     * with the corresponding exception as a cause.
     *
     * @param request The request message to send to the server.
     *
     * @param headers Metadata to attach to the request.  Most users will not need this.
     *
     * @return The single response from the server.
     */
    public suspend fun handle(request: EventBook, headers: Metadata = Metadata()): Empty = unaryRpc(
      channel,
      SagaCoordinatorGrpc.getHandleMethod(),
      request,
      callOptions,
      headers
    )

    /**
     * Executes this RPC and returns the response message, suspending until the RPC completes
     * with [`Status.OK`][io.grpc.Status].  If the RPC completes with another status, a
     * corresponding
     * [StatusException] is thrown.  If this coroutine is cancelled, the RPC is also cancelled
     * with the corresponding exception as a cause.
     *
     * @param request The request message to send to the server.
     *
     * @param headers Metadata to attach to the request.  Most users will not need this.
     *
     * @return The single response from the server.
     */
    public suspend fun handleSync(request: EventBook, headers: Metadata = Metadata()): SagaResponse
        = unaryRpc(
      channel,
      SagaCoordinatorGrpc.getHandleSyncMethod(),
      request,
      callOptions,
      headers
    )
  }

  /**
   * Skeletal implementation of the angzarr.SagaCoordinator service based on Kotlin coroutines.
   */
  public abstract class SagaCoordinatorCoroutineImplBase(
    coroutineContext: CoroutineContext = EmptyCoroutineContext,
  ) : AbstractCoroutineServerImpl(coroutineContext) {
    /**
     * Returns the response to an RPC for angzarr.SagaCoordinator.Handle.
     *
     * If this method fails with a [StatusException], the RPC will fail with the corresponding
     * [io.grpc.Status].  If this method fails with a [java.util.concurrent.CancellationException],
     * the RPC will fail
     * with status `Status.CANCELLED`.  If this method fails for any other reason, the RPC will
     * fail with `Status.UNKNOWN` with the exception as a cause.
     *
     * @param request The request from the client.
     */
    public open suspend fun handle(request: EventBook): Empty = throw
        StatusException(UNIMPLEMENTED.withDescription("Method angzarr.SagaCoordinator.Handle is unimplemented"))

    /**
     * Returns the response to an RPC for angzarr.SagaCoordinator.HandleSync.
     *
     * If this method fails with a [StatusException], the RPC will fail with the corresponding
     * [io.grpc.Status].  If this method fails with a [java.util.concurrent.CancellationException],
     * the RPC will fail
     * with status `Status.CANCELLED`.  If this method fails for any other reason, the RPC will
     * fail with `Status.UNKNOWN` with the exception as a cause.
     *
     * @param request The request from the client.
     */
    public open suspend fun handleSync(request: EventBook): SagaResponse = throw
        StatusException(UNIMPLEMENTED.withDescription("Method angzarr.SagaCoordinator.HandleSync is unimplemented"))

    final override fun bindService(): ServerServiceDefinition =
        builder(sagaCoordinatorGrpcGetServiceDescriptor())
      .addMethod(unaryServerMethodDefinition(
      context = this.context,
      descriptor = SagaCoordinatorGrpc.getHandleMethod(),
      implementation = ::handle
    ))
      .addMethod(unaryServerMethodDefinition(
      context = this.context,
      descriptor = SagaCoordinatorGrpc.getHandleSyncMethod(),
      implementation = ::handleSync
    )).build()
  }
}

/**
 * Holder for Kotlin coroutine-based client and server APIs for angzarr.Saga.
 */
public object SagaGrpcKt {
  public const val SERVICE_NAME: String = SagaGrpc.SERVICE_NAME

  @JvmStatic
  public val serviceDescriptor: ServiceDescriptor
    get() = sagaGrpcGetServiceDescriptor()

  public val handleMethod: MethodDescriptor<EventBook, Empty>
    @JvmStatic
    get() = SagaGrpc.getHandleMethod()

  public val handleSyncMethod: MethodDescriptor<EventBook, SagaResponse>
    @JvmStatic
    get() = SagaGrpc.getHandleSyncMethod()

  /**
   * A stub for issuing RPCs to a(n) angzarr.Saga service as suspending coroutines.
   */
  @StubFor(SagaGrpc::class)
  public class SagaCoroutineStub @JvmOverloads constructor(
    channel: Channel,
    callOptions: CallOptions = DEFAULT,
  ) : AbstractCoroutineStub<SagaCoroutineStub>(channel, callOptions) {
    override fun build(channel: Channel, callOptions: CallOptions): SagaCoroutineStub =
        SagaCoroutineStub(channel, callOptions)

    /**
     * Executes this RPC and returns the response message, suspending until the RPC completes
     * with [`Status.OK`][io.grpc.Status].  If the RPC completes with another status, a
     * corresponding
     * [StatusException] is thrown.  If this coroutine is cancelled, the RPC is also cancelled
     * with the corresponding exception as a cause.
     *
     * @param request The request message to send to the server.
     *
     * @param headers Metadata to attach to the request.  Most users will not need this.
     *
     * @return The single response from the server.
     */
    public suspend fun handle(request: EventBook, headers: Metadata = Metadata()): Empty = unaryRpc(
      channel,
      SagaGrpc.getHandleMethod(),
      request,
      callOptions,
      headers
    )

    /**
     * Executes this RPC and returns the response message, suspending until the RPC completes
     * with [`Status.OK`][io.grpc.Status].  If the RPC completes with another status, a
     * corresponding
     * [StatusException] is thrown.  If this coroutine is cancelled, the RPC is also cancelled
     * with the corresponding exception as a cause.
     *
     * @param request The request message to send to the server.
     *
     * @param headers Metadata to attach to the request.  Most users will not need this.
     *
     * @return The single response from the server.
     */
    public suspend fun handleSync(request: EventBook, headers: Metadata = Metadata()): SagaResponse
        = unaryRpc(
      channel,
      SagaGrpc.getHandleSyncMethod(),
      request,
      callOptions,
      headers
    )
  }

  /**
   * Skeletal implementation of the angzarr.Saga service based on Kotlin coroutines.
   */
  public abstract class SagaCoroutineImplBase(
    coroutineContext: CoroutineContext = EmptyCoroutineContext,
  ) : AbstractCoroutineServerImpl(coroutineContext) {
    /**
     * Returns the response to an RPC for angzarr.Saga.Handle.
     *
     * If this method fails with a [StatusException], the RPC will fail with the corresponding
     * [io.grpc.Status].  If this method fails with a [java.util.concurrent.CancellationException],
     * the RPC will fail
     * with status `Status.CANCELLED`.  If this method fails for any other reason, the RPC will
     * fail with `Status.UNKNOWN` with the exception as a cause.
     *
     * @param request The request from the client.
     */
    public open suspend fun handle(request: EventBook): Empty = throw
        StatusException(UNIMPLEMENTED.withDescription("Method angzarr.Saga.Handle is unimplemented"))

    /**
     * Returns the response to an RPC for angzarr.Saga.HandleSync.
     *
     * If this method fails with a [StatusException], the RPC will fail with the corresponding
     * [io.grpc.Status].  If this method fails with a [java.util.concurrent.CancellationException],
     * the RPC will fail
     * with status `Status.CANCELLED`.  If this method fails for any other reason, the RPC will
     * fail with `Status.UNKNOWN` with the exception as a cause.
     *
     * @param request The request from the client.
     */
    public open suspend fun handleSync(request: EventBook): SagaResponse = throw
        StatusException(UNIMPLEMENTED.withDescription("Method angzarr.Saga.HandleSync is unimplemented"))

    final override fun bindService(): ServerServiceDefinition =
        builder(sagaGrpcGetServiceDescriptor())
      .addMethod(unaryServerMethodDefinition(
      context = this.context,
      descriptor = SagaGrpc.getHandleMethod(),
      implementation = ::handle
    ))
      .addMethod(unaryServerMethodDefinition(
      context = this.context,
      descriptor = SagaGrpc.getHandleSyncMethod(),
      implementation = ::handleSync
    )).build()
  }
}

/**
 * Holder for Kotlin coroutine-based client and server APIs for angzarr.EventStream.
 */
public object EventStreamGrpcKt {
  public const val SERVICE_NAME: String = EventStreamGrpc.SERVICE_NAME

  @JvmStatic
  public val serviceDescriptor: ServiceDescriptor
    get() = eventStreamGrpcGetServiceDescriptor()

  public val subscribeMethod: MethodDescriptor<EventStreamFilter, EventBook>
    @JvmStatic
    get() = EventStreamGrpc.getSubscribeMethod()

  /**
   * A stub for issuing RPCs to a(n) angzarr.EventStream service as suspending coroutines.
   */
  @StubFor(EventStreamGrpc::class)
  public class EventStreamCoroutineStub @JvmOverloads constructor(
    channel: Channel,
    callOptions: CallOptions = DEFAULT,
  ) : AbstractCoroutineStub<EventStreamCoroutineStub>(channel, callOptions) {
    override fun build(channel: Channel, callOptions: CallOptions): EventStreamCoroutineStub =
        EventStreamCoroutineStub(channel, callOptions)

    /**
     * Returns a [Flow] that, when collected, executes this RPC and emits responses from the
     * server as they arrive.  That flow finishes normally if the server closes its response with
     * [`Status.OK`][io.grpc.Status], and fails by throwing a [StatusException] otherwise.  If
     * collecting the flow downstream fails exceptionally (including via cancellation), the RPC
     * is cancelled with that exception as a cause.
     *
     * @param request The request message to send to the server.
     *
     * @param headers Metadata to attach to the request.  Most users will not need this.
     *
     * @return A flow that, when collected, emits the responses from the server.
     */
    public fun subscribe(request: EventStreamFilter, headers: Metadata = Metadata()):
        Flow<EventBook> = serverStreamingRpc(
      channel,
      EventStreamGrpc.getSubscribeMethod(),
      request,
      callOptions,
      headers
    )
  }

  /**
   * Skeletal implementation of the angzarr.EventStream service based on Kotlin coroutines.
   */
  public abstract class EventStreamCoroutineImplBase(
    coroutineContext: CoroutineContext = EmptyCoroutineContext,
  ) : AbstractCoroutineServerImpl(coroutineContext) {
    /**
     * Returns a [Flow] of responses to an RPC for angzarr.EventStream.Subscribe.
     *
     * If creating or collecting the returned flow fails with a [StatusException], the RPC
     * will fail with the corresponding [io.grpc.Status].  If it fails with a
     * [java.util.concurrent.CancellationException], the RPC will fail with status
     * `Status.CANCELLED`.  If creating
     * or collecting the returned flow fails for any other reason, the RPC will fail with
     * `Status.UNKNOWN` with the exception as a cause.
     *
     * @param request The request from the client.
     */
    public open fun subscribe(request: EventStreamFilter): Flow<EventBook> = throw
        StatusException(UNIMPLEMENTED.withDescription("Method angzarr.EventStream.Subscribe is unimplemented"))

    final override fun bindService(): ServerServiceDefinition =
        builder(eventStreamGrpcGetServiceDescriptor())
      .addMethod(serverStreamingServerMethodDefinition(
      context = this.context,
      descriptor = EventStreamGrpc.getSubscribeMethod(),
      implementation = ::subscribe
    )).build()
  }
}

/**
 * Holder for Kotlin coroutine-based client and server APIs for angzarr.CommandProxy.
 */
public object CommandProxyGrpcKt {
  public const val SERVICE_NAME: String = CommandProxyGrpc.SERVICE_NAME

  @JvmStatic
  public val serviceDescriptor: ServiceDescriptor
    get() = commandProxyGrpcGetServiceDescriptor()

  public val executeMethod: MethodDescriptor<CommandBook, EventBook>
    @JvmStatic
    get() = CommandProxyGrpc.getExecuteMethod()

  /**
   * A stub for issuing RPCs to a(n) angzarr.CommandProxy service as suspending coroutines.
   */
  @StubFor(CommandProxyGrpc::class)
  public class CommandProxyCoroutineStub @JvmOverloads constructor(
    channel: Channel,
    callOptions: CallOptions = DEFAULT,
  ) : AbstractCoroutineStub<CommandProxyCoroutineStub>(channel, callOptions) {
    override fun build(channel: Channel, callOptions: CallOptions): CommandProxyCoroutineStub =
        CommandProxyCoroutineStub(channel, callOptions)

    /**
     * Returns a [Flow] that, when collected, executes this RPC and emits responses from the
     * server as they arrive.  That flow finishes normally if the server closes its response with
     * [`Status.OK`][io.grpc.Status], and fails by throwing a [StatusException] otherwise.  If
     * collecting the flow downstream fails exceptionally (including via cancellation), the RPC
     * is cancelled with that exception as a cause.
     *
     * @param request The request message to send to the server.
     *
     * @param headers Metadata to attach to the request.  Most users will not need this.
     *
     * @return A flow that, when collected, emits the responses from the server.
     */
    public fun execute(request: CommandBook, headers: Metadata = Metadata()): Flow<EventBook> =
        serverStreamingRpc(
      channel,
      CommandProxyGrpc.getExecuteMethod(),
      request,
      callOptions,
      headers
    )
  }

  /**
   * Skeletal implementation of the angzarr.CommandProxy service based on Kotlin coroutines.
   */
  public abstract class CommandProxyCoroutineImplBase(
    coroutineContext: CoroutineContext = EmptyCoroutineContext,
  ) : AbstractCoroutineServerImpl(coroutineContext) {
    /**
     * Returns a [Flow] of responses to an RPC for angzarr.CommandProxy.Execute.
     *
     * If creating or collecting the returned flow fails with a [StatusException], the RPC
     * will fail with the corresponding [io.grpc.Status].  If it fails with a
     * [java.util.concurrent.CancellationException], the RPC will fail with status
     * `Status.CANCELLED`.  If creating
     * or collecting the returned flow fails for any other reason, the RPC will fail with
     * `Status.UNKNOWN` with the exception as a cause.
     *
     * @param request The request from the client.
     */
    public open fun execute(request: CommandBook): Flow<EventBook> = throw
        StatusException(UNIMPLEMENTED.withDescription("Method angzarr.CommandProxy.Execute is unimplemented"))

    final override fun bindService(): ServerServiceDefinition =
        builder(commandProxyGrpcGetServiceDescriptor())
      .addMethod(serverStreamingServerMethodDefinition(
      context = this.context,
      descriptor = CommandProxyGrpc.getExecuteMethod(),
      implementation = ::execute
    )).build()
  }
}
