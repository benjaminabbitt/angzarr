package dev.angzarr;

import static io.grpc.MethodDescriptor.generateFullMethodName;

/**
 */
@javax.annotation.Generated(
    value = "by gRPC proto compiler (version 1.60.0)",
    comments = "Source: angzarr/angzarr.proto")
@io.grpc.stub.annotations.GrpcGenerated
public final class EventQueryGrpc {

  private EventQueryGrpc() {}

  public static final java.lang.String SERVICE_NAME = "angzarr.EventQuery";

  // Static method descriptors that strictly reflect the proto.
  private static volatile io.grpc.MethodDescriptor<dev.angzarr.Query,
      dev.angzarr.EventBook> getGetEventBookMethod;

  @io.grpc.stub.annotations.RpcMethod(
      fullMethodName = SERVICE_NAME + '/' + "GetEventBook",
      requestType = dev.angzarr.Query.class,
      responseType = dev.angzarr.EventBook.class,
      methodType = io.grpc.MethodDescriptor.MethodType.UNARY)
  public static io.grpc.MethodDescriptor<dev.angzarr.Query,
      dev.angzarr.EventBook> getGetEventBookMethod() {
    io.grpc.MethodDescriptor<dev.angzarr.Query, dev.angzarr.EventBook> getGetEventBookMethod;
    if ((getGetEventBookMethod = EventQueryGrpc.getGetEventBookMethod) == null) {
      synchronized (EventQueryGrpc.class) {
        if ((getGetEventBookMethod = EventQueryGrpc.getGetEventBookMethod) == null) {
          EventQueryGrpc.getGetEventBookMethod = getGetEventBookMethod =
              io.grpc.MethodDescriptor.<dev.angzarr.Query, dev.angzarr.EventBook>newBuilder()
              .setType(io.grpc.MethodDescriptor.MethodType.UNARY)
              .setFullMethodName(generateFullMethodName(SERVICE_NAME, "GetEventBook"))
              .setSampledToLocalTracing(true)
              .setRequestMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.Query.getDefaultInstance()))
              .setResponseMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.EventBook.getDefaultInstance()))
              .setSchemaDescriptor(new EventQueryMethodDescriptorSupplier("GetEventBook"))
              .build();
        }
      }
    }
    return getGetEventBookMethod;
  }

  private static volatile io.grpc.MethodDescriptor<dev.angzarr.Query,
      dev.angzarr.EventBook> getGetEventsMethod;

  @io.grpc.stub.annotations.RpcMethod(
      fullMethodName = SERVICE_NAME + '/' + "GetEvents",
      requestType = dev.angzarr.Query.class,
      responseType = dev.angzarr.EventBook.class,
      methodType = io.grpc.MethodDescriptor.MethodType.SERVER_STREAMING)
  public static io.grpc.MethodDescriptor<dev.angzarr.Query,
      dev.angzarr.EventBook> getGetEventsMethod() {
    io.grpc.MethodDescriptor<dev.angzarr.Query, dev.angzarr.EventBook> getGetEventsMethod;
    if ((getGetEventsMethod = EventQueryGrpc.getGetEventsMethod) == null) {
      synchronized (EventQueryGrpc.class) {
        if ((getGetEventsMethod = EventQueryGrpc.getGetEventsMethod) == null) {
          EventQueryGrpc.getGetEventsMethod = getGetEventsMethod =
              io.grpc.MethodDescriptor.<dev.angzarr.Query, dev.angzarr.EventBook>newBuilder()
              .setType(io.grpc.MethodDescriptor.MethodType.SERVER_STREAMING)
              .setFullMethodName(generateFullMethodName(SERVICE_NAME, "GetEvents"))
              .setSampledToLocalTracing(true)
              .setRequestMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.Query.getDefaultInstance()))
              .setResponseMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.EventBook.getDefaultInstance()))
              .setSchemaDescriptor(new EventQueryMethodDescriptorSupplier("GetEvents"))
              .build();
        }
      }
    }
    return getGetEventsMethod;
  }

  private static volatile io.grpc.MethodDescriptor<dev.angzarr.Query,
      dev.angzarr.EventBook> getSynchronizeMethod;

  @io.grpc.stub.annotations.RpcMethod(
      fullMethodName = SERVICE_NAME + '/' + "Synchronize",
      requestType = dev.angzarr.Query.class,
      responseType = dev.angzarr.EventBook.class,
      methodType = io.grpc.MethodDescriptor.MethodType.BIDI_STREAMING)
  public static io.grpc.MethodDescriptor<dev.angzarr.Query,
      dev.angzarr.EventBook> getSynchronizeMethod() {
    io.grpc.MethodDescriptor<dev.angzarr.Query, dev.angzarr.EventBook> getSynchronizeMethod;
    if ((getSynchronizeMethod = EventQueryGrpc.getSynchronizeMethod) == null) {
      synchronized (EventQueryGrpc.class) {
        if ((getSynchronizeMethod = EventQueryGrpc.getSynchronizeMethod) == null) {
          EventQueryGrpc.getSynchronizeMethod = getSynchronizeMethod =
              io.grpc.MethodDescriptor.<dev.angzarr.Query, dev.angzarr.EventBook>newBuilder()
              .setType(io.grpc.MethodDescriptor.MethodType.BIDI_STREAMING)
              .setFullMethodName(generateFullMethodName(SERVICE_NAME, "Synchronize"))
              .setSampledToLocalTracing(true)
              .setRequestMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.Query.getDefaultInstance()))
              .setResponseMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.EventBook.getDefaultInstance()))
              .setSchemaDescriptor(new EventQueryMethodDescriptorSupplier("Synchronize"))
              .build();
        }
      }
    }
    return getSynchronizeMethod;
  }

  private static volatile io.grpc.MethodDescriptor<com.google.protobuf.Empty,
      dev.angzarr.AggregateRoot> getGetAggregateRootsMethod;

  @io.grpc.stub.annotations.RpcMethod(
      fullMethodName = SERVICE_NAME + '/' + "GetAggregateRoots",
      requestType = com.google.protobuf.Empty.class,
      responseType = dev.angzarr.AggregateRoot.class,
      methodType = io.grpc.MethodDescriptor.MethodType.SERVER_STREAMING)
  public static io.grpc.MethodDescriptor<com.google.protobuf.Empty,
      dev.angzarr.AggregateRoot> getGetAggregateRootsMethod() {
    io.grpc.MethodDescriptor<com.google.protobuf.Empty, dev.angzarr.AggregateRoot> getGetAggregateRootsMethod;
    if ((getGetAggregateRootsMethod = EventQueryGrpc.getGetAggregateRootsMethod) == null) {
      synchronized (EventQueryGrpc.class) {
        if ((getGetAggregateRootsMethod = EventQueryGrpc.getGetAggregateRootsMethod) == null) {
          EventQueryGrpc.getGetAggregateRootsMethod = getGetAggregateRootsMethod =
              io.grpc.MethodDescriptor.<com.google.protobuf.Empty, dev.angzarr.AggregateRoot>newBuilder()
              .setType(io.grpc.MethodDescriptor.MethodType.SERVER_STREAMING)
              .setFullMethodName(generateFullMethodName(SERVICE_NAME, "GetAggregateRoots"))
              .setSampledToLocalTracing(true)
              .setRequestMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  com.google.protobuf.Empty.getDefaultInstance()))
              .setResponseMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.AggregateRoot.getDefaultInstance()))
              .setSchemaDescriptor(new EventQueryMethodDescriptorSupplier("GetAggregateRoots"))
              .build();
        }
      }
    }
    return getGetAggregateRootsMethod;
  }

  /**
   * Creates a new async stub that supports all call types for the service
   */
  public static EventQueryStub newStub(io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<EventQueryStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<EventQueryStub>() {
        @java.lang.Override
        public EventQueryStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new EventQueryStub(channel, callOptions);
        }
      };
    return EventQueryStub.newStub(factory, channel);
  }

  /**
   * Creates a new blocking-style stub that supports unary and streaming output calls on the service
   */
  public static EventQueryBlockingStub newBlockingStub(
      io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<EventQueryBlockingStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<EventQueryBlockingStub>() {
        @java.lang.Override
        public EventQueryBlockingStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new EventQueryBlockingStub(channel, callOptions);
        }
      };
    return EventQueryBlockingStub.newStub(factory, channel);
  }

  /**
   * Creates a new ListenableFuture-style stub that supports unary calls on the service
   */
  public static EventQueryFutureStub newFutureStub(
      io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<EventQueryFutureStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<EventQueryFutureStub>() {
        @java.lang.Override
        public EventQueryFutureStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new EventQueryFutureStub(channel, callOptions);
        }
      };
    return EventQueryFutureStub.newStub(factory, channel);
  }

  /**
   */
  public interface AsyncService {

    /**
     * <pre>
     * Get a single EventBook (unary) - use for explicit queries with gRPC tooling
     * </pre>
     */
    default void getEventBook(dev.angzarr.Query request,
        io.grpc.stub.StreamObserver<dev.angzarr.EventBook> responseObserver) {
      io.grpc.stub.ServerCalls.asyncUnimplementedUnaryCall(getGetEventBookMethod(), responseObserver);
    }

    /**
     * <pre>
     * Stream EventBooks matching query - use for bulk retrieval
     * </pre>
     */
    default void getEvents(dev.angzarr.Query request,
        io.grpc.stub.StreamObserver<dev.angzarr.EventBook> responseObserver) {
      io.grpc.stub.ServerCalls.asyncUnimplementedUnaryCall(getGetEventsMethod(), responseObserver);
    }

    /**
     */
    default io.grpc.stub.StreamObserver<dev.angzarr.Query> synchronize(
        io.grpc.stub.StreamObserver<dev.angzarr.EventBook> responseObserver) {
      return io.grpc.stub.ServerCalls.asyncUnimplementedStreamingCall(getSynchronizeMethod(), responseObserver);
    }

    /**
     */
    default void getAggregateRoots(com.google.protobuf.Empty request,
        io.grpc.stub.StreamObserver<dev.angzarr.AggregateRoot> responseObserver) {
      io.grpc.stub.ServerCalls.asyncUnimplementedUnaryCall(getGetAggregateRootsMethod(), responseObserver);
    }
  }

  /**
   * Base class for the server implementation of the service EventQuery.
   */
  public static abstract class EventQueryImplBase
      implements io.grpc.BindableService, AsyncService {

    @java.lang.Override public final io.grpc.ServerServiceDefinition bindService() {
      return EventQueryGrpc.bindService(this);
    }
  }

  /**
   * A stub to allow clients to do asynchronous rpc calls to service EventQuery.
   */
  public static final class EventQueryStub
      extends io.grpc.stub.AbstractAsyncStub<EventQueryStub> {
    private EventQueryStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected EventQueryStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new EventQueryStub(channel, callOptions);
    }

    /**
     * <pre>
     * Get a single EventBook (unary) - use for explicit queries with gRPC tooling
     * </pre>
     */
    public void getEventBook(dev.angzarr.Query request,
        io.grpc.stub.StreamObserver<dev.angzarr.EventBook> responseObserver) {
      io.grpc.stub.ClientCalls.asyncUnaryCall(
          getChannel().newCall(getGetEventBookMethod(), getCallOptions()), request, responseObserver);
    }

    /**
     * <pre>
     * Stream EventBooks matching query - use for bulk retrieval
     * </pre>
     */
    public void getEvents(dev.angzarr.Query request,
        io.grpc.stub.StreamObserver<dev.angzarr.EventBook> responseObserver) {
      io.grpc.stub.ClientCalls.asyncServerStreamingCall(
          getChannel().newCall(getGetEventsMethod(), getCallOptions()), request, responseObserver);
    }

    /**
     */
    public io.grpc.stub.StreamObserver<dev.angzarr.Query> synchronize(
        io.grpc.stub.StreamObserver<dev.angzarr.EventBook> responseObserver) {
      return io.grpc.stub.ClientCalls.asyncBidiStreamingCall(
          getChannel().newCall(getSynchronizeMethod(), getCallOptions()), responseObserver);
    }

    /**
     */
    public void getAggregateRoots(com.google.protobuf.Empty request,
        io.grpc.stub.StreamObserver<dev.angzarr.AggregateRoot> responseObserver) {
      io.grpc.stub.ClientCalls.asyncServerStreamingCall(
          getChannel().newCall(getGetAggregateRootsMethod(), getCallOptions()), request, responseObserver);
    }
  }

  /**
   * A stub to allow clients to do synchronous rpc calls to service EventQuery.
   */
  public static final class EventQueryBlockingStub
      extends io.grpc.stub.AbstractBlockingStub<EventQueryBlockingStub> {
    private EventQueryBlockingStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected EventQueryBlockingStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new EventQueryBlockingStub(channel, callOptions);
    }

    /**
     * <pre>
     * Get a single EventBook (unary) - use for explicit queries with gRPC tooling
     * </pre>
     */
    public dev.angzarr.EventBook getEventBook(dev.angzarr.Query request) {
      return io.grpc.stub.ClientCalls.blockingUnaryCall(
          getChannel(), getGetEventBookMethod(), getCallOptions(), request);
    }

    /**
     * <pre>
     * Stream EventBooks matching query - use for bulk retrieval
     * </pre>
     */
    public java.util.Iterator<dev.angzarr.EventBook> getEvents(
        dev.angzarr.Query request) {
      return io.grpc.stub.ClientCalls.blockingServerStreamingCall(
          getChannel(), getGetEventsMethod(), getCallOptions(), request);
    }

    /**
     */
    public java.util.Iterator<dev.angzarr.AggregateRoot> getAggregateRoots(
        com.google.protobuf.Empty request) {
      return io.grpc.stub.ClientCalls.blockingServerStreamingCall(
          getChannel(), getGetAggregateRootsMethod(), getCallOptions(), request);
    }
  }

  /**
   * A stub to allow clients to do ListenableFuture-style rpc calls to service EventQuery.
   */
  public static final class EventQueryFutureStub
      extends io.grpc.stub.AbstractFutureStub<EventQueryFutureStub> {
    private EventQueryFutureStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected EventQueryFutureStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new EventQueryFutureStub(channel, callOptions);
    }

    /**
     * <pre>
     * Get a single EventBook (unary) - use for explicit queries with gRPC tooling
     * </pre>
     */
    public com.google.common.util.concurrent.ListenableFuture<dev.angzarr.EventBook> getEventBook(
        dev.angzarr.Query request) {
      return io.grpc.stub.ClientCalls.futureUnaryCall(
          getChannel().newCall(getGetEventBookMethod(), getCallOptions()), request);
    }
  }

  private static final int METHODID_GET_EVENT_BOOK = 0;
  private static final int METHODID_GET_EVENTS = 1;
  private static final int METHODID_GET_AGGREGATE_ROOTS = 2;
  private static final int METHODID_SYNCHRONIZE = 3;

  private static final class MethodHandlers<Req, Resp> implements
      io.grpc.stub.ServerCalls.UnaryMethod<Req, Resp>,
      io.grpc.stub.ServerCalls.ServerStreamingMethod<Req, Resp>,
      io.grpc.stub.ServerCalls.ClientStreamingMethod<Req, Resp>,
      io.grpc.stub.ServerCalls.BidiStreamingMethod<Req, Resp> {
    private final AsyncService serviceImpl;
    private final int methodId;

    MethodHandlers(AsyncService serviceImpl, int methodId) {
      this.serviceImpl = serviceImpl;
      this.methodId = methodId;
    }

    @java.lang.Override
    @java.lang.SuppressWarnings("unchecked")
    public void invoke(Req request, io.grpc.stub.StreamObserver<Resp> responseObserver) {
      switch (methodId) {
        case METHODID_GET_EVENT_BOOK:
          serviceImpl.getEventBook((dev.angzarr.Query) request,
              (io.grpc.stub.StreamObserver<dev.angzarr.EventBook>) responseObserver);
          break;
        case METHODID_GET_EVENTS:
          serviceImpl.getEvents((dev.angzarr.Query) request,
              (io.grpc.stub.StreamObserver<dev.angzarr.EventBook>) responseObserver);
          break;
        case METHODID_GET_AGGREGATE_ROOTS:
          serviceImpl.getAggregateRoots((com.google.protobuf.Empty) request,
              (io.grpc.stub.StreamObserver<dev.angzarr.AggregateRoot>) responseObserver);
          break;
        default:
          throw new AssertionError();
      }
    }

    @java.lang.Override
    @java.lang.SuppressWarnings("unchecked")
    public io.grpc.stub.StreamObserver<Req> invoke(
        io.grpc.stub.StreamObserver<Resp> responseObserver) {
      switch (methodId) {
        case METHODID_SYNCHRONIZE:
          return (io.grpc.stub.StreamObserver<Req>) serviceImpl.synchronize(
              (io.grpc.stub.StreamObserver<dev.angzarr.EventBook>) responseObserver);
        default:
          throw new AssertionError();
      }
    }
  }

  public static final io.grpc.ServerServiceDefinition bindService(AsyncService service) {
    return io.grpc.ServerServiceDefinition.builder(getServiceDescriptor())
        .addMethod(
          getGetEventBookMethod(),
          io.grpc.stub.ServerCalls.asyncUnaryCall(
            new MethodHandlers<
              dev.angzarr.Query,
              dev.angzarr.EventBook>(
                service, METHODID_GET_EVENT_BOOK)))
        .addMethod(
          getGetEventsMethod(),
          io.grpc.stub.ServerCalls.asyncServerStreamingCall(
            new MethodHandlers<
              dev.angzarr.Query,
              dev.angzarr.EventBook>(
                service, METHODID_GET_EVENTS)))
        .addMethod(
          getSynchronizeMethod(),
          io.grpc.stub.ServerCalls.asyncBidiStreamingCall(
            new MethodHandlers<
              dev.angzarr.Query,
              dev.angzarr.EventBook>(
                service, METHODID_SYNCHRONIZE)))
        .addMethod(
          getGetAggregateRootsMethod(),
          io.grpc.stub.ServerCalls.asyncServerStreamingCall(
            new MethodHandlers<
              com.google.protobuf.Empty,
              dev.angzarr.AggregateRoot>(
                service, METHODID_GET_AGGREGATE_ROOTS)))
        .build();
  }

  private static abstract class EventQueryBaseDescriptorSupplier
      implements io.grpc.protobuf.ProtoFileDescriptorSupplier, io.grpc.protobuf.ProtoServiceDescriptorSupplier {
    EventQueryBaseDescriptorSupplier() {}

    @java.lang.Override
    public com.google.protobuf.Descriptors.FileDescriptor getFileDescriptor() {
      return dev.angzarr.Angzarr.getDescriptor();
    }

    @java.lang.Override
    public com.google.protobuf.Descriptors.ServiceDescriptor getServiceDescriptor() {
      return getFileDescriptor().findServiceByName("EventQuery");
    }
  }

  private static final class EventQueryFileDescriptorSupplier
      extends EventQueryBaseDescriptorSupplier {
    EventQueryFileDescriptorSupplier() {}
  }

  private static final class EventQueryMethodDescriptorSupplier
      extends EventQueryBaseDescriptorSupplier
      implements io.grpc.protobuf.ProtoMethodDescriptorSupplier {
    private final java.lang.String methodName;

    EventQueryMethodDescriptorSupplier(java.lang.String methodName) {
      this.methodName = methodName;
    }

    @java.lang.Override
    public com.google.protobuf.Descriptors.MethodDescriptor getMethodDescriptor() {
      return getServiceDescriptor().findMethodByName(methodName);
    }
  }

  private static volatile io.grpc.ServiceDescriptor serviceDescriptor;

  public static io.grpc.ServiceDescriptor getServiceDescriptor() {
    io.grpc.ServiceDescriptor result = serviceDescriptor;
    if (result == null) {
      synchronized (EventQueryGrpc.class) {
        result = serviceDescriptor;
        if (result == null) {
          serviceDescriptor = result = io.grpc.ServiceDescriptor.newBuilder(SERVICE_NAME)
              .setSchemaDescriptor(new EventQueryFileDescriptorSupplier())
              .addMethod(getGetEventBookMethod())
              .addMethod(getGetEventsMethod())
              .addMethod(getSynchronizeMethod())
              .addMethod(getGetAggregateRootsMethod())
              .build();
        }
      }
    }
    return result;
  }
}
