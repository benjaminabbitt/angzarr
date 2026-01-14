package dev.angzarr;

import static io.grpc.MethodDescriptor.generateFullMethodName;

/**
 * <pre>
 * Event stream service - streams events to registered subscribers
 * </pre>
 */
@javax.annotation.Generated(
    value = "by gRPC proto compiler (version 1.60.0)",
    comments = "Source: angzarr/angzarr.proto")
@io.grpc.stub.annotations.GrpcGenerated
public final class EventStreamGrpc {

  private EventStreamGrpc() {}

  public static final java.lang.String SERVICE_NAME = "angzarr.EventStream";

  // Static method descriptors that strictly reflect the proto.
  private static volatile io.grpc.MethodDescriptor<dev.angzarr.EventStreamFilter,
      dev.angzarr.EventBook> getSubscribeMethod;

  @io.grpc.stub.annotations.RpcMethod(
      fullMethodName = SERVICE_NAME + '/' + "Subscribe",
      requestType = dev.angzarr.EventStreamFilter.class,
      responseType = dev.angzarr.EventBook.class,
      methodType = io.grpc.MethodDescriptor.MethodType.SERVER_STREAMING)
  public static io.grpc.MethodDescriptor<dev.angzarr.EventStreamFilter,
      dev.angzarr.EventBook> getSubscribeMethod() {
    io.grpc.MethodDescriptor<dev.angzarr.EventStreamFilter, dev.angzarr.EventBook> getSubscribeMethod;
    if ((getSubscribeMethod = EventStreamGrpc.getSubscribeMethod) == null) {
      synchronized (EventStreamGrpc.class) {
        if ((getSubscribeMethod = EventStreamGrpc.getSubscribeMethod) == null) {
          EventStreamGrpc.getSubscribeMethod = getSubscribeMethod =
              io.grpc.MethodDescriptor.<dev.angzarr.EventStreamFilter, dev.angzarr.EventBook>newBuilder()
              .setType(io.grpc.MethodDescriptor.MethodType.SERVER_STREAMING)
              .setFullMethodName(generateFullMethodName(SERVICE_NAME, "Subscribe"))
              .setSampledToLocalTracing(true)
              .setRequestMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.EventStreamFilter.getDefaultInstance()))
              .setResponseMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.EventBook.getDefaultInstance()))
              .setSchemaDescriptor(new EventStreamMethodDescriptorSupplier("Subscribe"))
              .build();
        }
      }
    }
    return getSubscribeMethod;
  }

  /**
   * Creates a new async stub that supports all call types for the service
   */
  public static EventStreamStub newStub(io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<EventStreamStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<EventStreamStub>() {
        @java.lang.Override
        public EventStreamStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new EventStreamStub(channel, callOptions);
        }
      };
    return EventStreamStub.newStub(factory, channel);
  }

  /**
   * Creates a new blocking-style stub that supports unary and streaming output calls on the service
   */
  public static EventStreamBlockingStub newBlockingStub(
      io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<EventStreamBlockingStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<EventStreamBlockingStub>() {
        @java.lang.Override
        public EventStreamBlockingStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new EventStreamBlockingStub(channel, callOptions);
        }
      };
    return EventStreamBlockingStub.newStub(factory, channel);
  }

  /**
   * Creates a new ListenableFuture-style stub that supports unary calls on the service
   */
  public static EventStreamFutureStub newFutureStub(
      io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<EventStreamFutureStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<EventStreamFutureStub>() {
        @java.lang.Override
        public EventStreamFutureStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new EventStreamFutureStub(channel, callOptions);
        }
      };
    return EventStreamFutureStub.newStub(factory, channel);
  }

  /**
   * <pre>
   * Event stream service - streams events to registered subscribers
   * </pre>
   */
  public interface AsyncService {

    /**
     * <pre>
     * Subscribe to events matching correlation ID (required)
     * Returns INVALID_ARGUMENT if correlation_id is empty
     * </pre>
     */
    default void subscribe(dev.angzarr.EventStreamFilter request,
        io.grpc.stub.StreamObserver<dev.angzarr.EventBook> responseObserver) {
      io.grpc.stub.ServerCalls.asyncUnimplementedUnaryCall(getSubscribeMethod(), responseObserver);
    }
  }

  /**
   * Base class for the server implementation of the service EventStream.
   * <pre>
   * Event stream service - streams events to registered subscribers
   * </pre>
   */
  public static abstract class EventStreamImplBase
      implements io.grpc.BindableService, AsyncService {

    @java.lang.Override public final io.grpc.ServerServiceDefinition bindService() {
      return EventStreamGrpc.bindService(this);
    }
  }

  /**
   * A stub to allow clients to do asynchronous rpc calls to service EventStream.
   * <pre>
   * Event stream service - streams events to registered subscribers
   * </pre>
   */
  public static final class EventStreamStub
      extends io.grpc.stub.AbstractAsyncStub<EventStreamStub> {
    private EventStreamStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected EventStreamStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new EventStreamStub(channel, callOptions);
    }

    /**
     * <pre>
     * Subscribe to events matching correlation ID (required)
     * Returns INVALID_ARGUMENT if correlation_id is empty
     * </pre>
     */
    public void subscribe(dev.angzarr.EventStreamFilter request,
        io.grpc.stub.StreamObserver<dev.angzarr.EventBook> responseObserver) {
      io.grpc.stub.ClientCalls.asyncServerStreamingCall(
          getChannel().newCall(getSubscribeMethod(), getCallOptions()), request, responseObserver);
    }
  }

  /**
   * A stub to allow clients to do synchronous rpc calls to service EventStream.
   * <pre>
   * Event stream service - streams events to registered subscribers
   * </pre>
   */
  public static final class EventStreamBlockingStub
      extends io.grpc.stub.AbstractBlockingStub<EventStreamBlockingStub> {
    private EventStreamBlockingStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected EventStreamBlockingStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new EventStreamBlockingStub(channel, callOptions);
    }

    /**
     * <pre>
     * Subscribe to events matching correlation ID (required)
     * Returns INVALID_ARGUMENT if correlation_id is empty
     * </pre>
     */
    public java.util.Iterator<dev.angzarr.EventBook> subscribe(
        dev.angzarr.EventStreamFilter request) {
      return io.grpc.stub.ClientCalls.blockingServerStreamingCall(
          getChannel(), getSubscribeMethod(), getCallOptions(), request);
    }
  }

  /**
   * A stub to allow clients to do ListenableFuture-style rpc calls to service EventStream.
   * <pre>
   * Event stream service - streams events to registered subscribers
   * </pre>
   */
  public static final class EventStreamFutureStub
      extends io.grpc.stub.AbstractFutureStub<EventStreamFutureStub> {
    private EventStreamFutureStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected EventStreamFutureStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new EventStreamFutureStub(channel, callOptions);
    }
  }

  private static final int METHODID_SUBSCRIBE = 0;

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
        case METHODID_SUBSCRIBE:
          serviceImpl.subscribe((dev.angzarr.EventStreamFilter) request,
              (io.grpc.stub.StreamObserver<dev.angzarr.EventBook>) responseObserver);
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
        default:
          throw new AssertionError();
      }
    }
  }

  public static final io.grpc.ServerServiceDefinition bindService(AsyncService service) {
    return io.grpc.ServerServiceDefinition.builder(getServiceDescriptor())
        .addMethod(
          getSubscribeMethod(),
          io.grpc.stub.ServerCalls.asyncServerStreamingCall(
            new MethodHandlers<
              dev.angzarr.EventStreamFilter,
              dev.angzarr.EventBook>(
                service, METHODID_SUBSCRIBE)))
        .build();
  }

  private static abstract class EventStreamBaseDescriptorSupplier
      implements io.grpc.protobuf.ProtoFileDescriptorSupplier, io.grpc.protobuf.ProtoServiceDescriptorSupplier {
    EventStreamBaseDescriptorSupplier() {}

    @java.lang.Override
    public com.google.protobuf.Descriptors.FileDescriptor getFileDescriptor() {
      return dev.angzarr.Angzarr.getDescriptor();
    }

    @java.lang.Override
    public com.google.protobuf.Descriptors.ServiceDescriptor getServiceDescriptor() {
      return getFileDescriptor().findServiceByName("EventStream");
    }
  }

  private static final class EventStreamFileDescriptorSupplier
      extends EventStreamBaseDescriptorSupplier {
    EventStreamFileDescriptorSupplier() {}
  }

  private static final class EventStreamMethodDescriptorSupplier
      extends EventStreamBaseDescriptorSupplier
      implements io.grpc.protobuf.ProtoMethodDescriptorSupplier {
    private final java.lang.String methodName;

    EventStreamMethodDescriptorSupplier(java.lang.String methodName) {
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
      synchronized (EventStreamGrpc.class) {
        result = serviceDescriptor;
        if (result == null) {
          serviceDescriptor = result = io.grpc.ServiceDescriptor.newBuilder(SERVICE_NAME)
              .setSchemaDescriptor(new EventStreamFileDescriptorSupplier())
              .addMethod(getSubscribeMethod())
              .build();
        }
      }
    }
    return result;
  }
}
