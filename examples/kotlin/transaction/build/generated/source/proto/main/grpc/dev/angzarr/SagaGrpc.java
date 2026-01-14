package dev.angzarr;

import static io.grpc.MethodDescriptor.generateFullMethodName;

/**
 */
@javax.annotation.Generated(
    value = "by gRPC proto compiler (version 1.60.0)",
    comments = "Source: angzarr/angzarr.proto")
@io.grpc.stub.annotations.GrpcGenerated
public final class SagaGrpc {

  private SagaGrpc() {}

  public static final java.lang.String SERVICE_NAME = "angzarr.Saga";

  // Static method descriptors that strictly reflect the proto.
  private static volatile io.grpc.MethodDescriptor<dev.angzarr.EventBook,
      com.google.protobuf.Empty> getHandleMethod;

  @io.grpc.stub.annotations.RpcMethod(
      fullMethodName = SERVICE_NAME + '/' + "Handle",
      requestType = dev.angzarr.EventBook.class,
      responseType = com.google.protobuf.Empty.class,
      methodType = io.grpc.MethodDescriptor.MethodType.UNARY)
  public static io.grpc.MethodDescriptor<dev.angzarr.EventBook,
      com.google.protobuf.Empty> getHandleMethod() {
    io.grpc.MethodDescriptor<dev.angzarr.EventBook, com.google.protobuf.Empty> getHandleMethod;
    if ((getHandleMethod = SagaGrpc.getHandleMethod) == null) {
      synchronized (SagaGrpc.class) {
        if ((getHandleMethod = SagaGrpc.getHandleMethod) == null) {
          SagaGrpc.getHandleMethod = getHandleMethod =
              io.grpc.MethodDescriptor.<dev.angzarr.EventBook, com.google.protobuf.Empty>newBuilder()
              .setType(io.grpc.MethodDescriptor.MethodType.UNARY)
              .setFullMethodName(generateFullMethodName(SERVICE_NAME, "Handle"))
              .setSampledToLocalTracing(true)
              .setRequestMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.EventBook.getDefaultInstance()))
              .setResponseMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  com.google.protobuf.Empty.getDefaultInstance()))
              .setSchemaDescriptor(new SagaMethodDescriptorSupplier("Handle"))
              .build();
        }
      }
    }
    return getHandleMethod;
  }

  private static volatile io.grpc.MethodDescriptor<dev.angzarr.EventBook,
      dev.angzarr.SagaResponse> getHandleSyncMethod;

  @io.grpc.stub.annotations.RpcMethod(
      fullMethodName = SERVICE_NAME + '/' + "HandleSync",
      requestType = dev.angzarr.EventBook.class,
      responseType = dev.angzarr.SagaResponse.class,
      methodType = io.grpc.MethodDescriptor.MethodType.UNARY)
  public static io.grpc.MethodDescriptor<dev.angzarr.EventBook,
      dev.angzarr.SagaResponse> getHandleSyncMethod() {
    io.grpc.MethodDescriptor<dev.angzarr.EventBook, dev.angzarr.SagaResponse> getHandleSyncMethod;
    if ((getHandleSyncMethod = SagaGrpc.getHandleSyncMethod) == null) {
      synchronized (SagaGrpc.class) {
        if ((getHandleSyncMethod = SagaGrpc.getHandleSyncMethod) == null) {
          SagaGrpc.getHandleSyncMethod = getHandleSyncMethod =
              io.grpc.MethodDescriptor.<dev.angzarr.EventBook, dev.angzarr.SagaResponse>newBuilder()
              .setType(io.grpc.MethodDescriptor.MethodType.UNARY)
              .setFullMethodName(generateFullMethodName(SERVICE_NAME, "HandleSync"))
              .setSampledToLocalTracing(true)
              .setRequestMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.EventBook.getDefaultInstance()))
              .setResponseMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.SagaResponse.getDefaultInstance()))
              .setSchemaDescriptor(new SagaMethodDescriptorSupplier("HandleSync"))
              .build();
        }
      }
    }
    return getHandleSyncMethod;
  }

  /**
   * Creates a new async stub that supports all call types for the service
   */
  public static SagaStub newStub(io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<SagaStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<SagaStub>() {
        @java.lang.Override
        public SagaStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new SagaStub(channel, callOptions);
        }
      };
    return SagaStub.newStub(factory, channel);
  }

  /**
   * Creates a new blocking-style stub that supports unary and streaming output calls on the service
   */
  public static SagaBlockingStub newBlockingStub(
      io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<SagaBlockingStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<SagaBlockingStub>() {
        @java.lang.Override
        public SagaBlockingStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new SagaBlockingStub(channel, callOptions);
        }
      };
    return SagaBlockingStub.newStub(factory, channel);
  }

  /**
   * Creates a new ListenableFuture-style stub that supports unary calls on the service
   */
  public static SagaFutureStub newFutureStub(
      io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<SagaFutureStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<SagaFutureStub>() {
        @java.lang.Override
        public SagaFutureStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new SagaFutureStub(channel, callOptions);
        }
      };
    return SagaFutureStub.newStub(factory, channel);
  }

  /**
   */
  public interface AsyncService {

    /**
     */
    default void handle(dev.angzarr.EventBook request,
        io.grpc.stub.StreamObserver<com.google.protobuf.Empty> responseObserver) {
      io.grpc.stub.ServerCalls.asyncUnimplementedUnaryCall(getHandleMethod(), responseObserver);
    }

    /**
     */
    default void handleSync(dev.angzarr.EventBook request,
        io.grpc.stub.StreamObserver<dev.angzarr.SagaResponse> responseObserver) {
      io.grpc.stub.ServerCalls.asyncUnimplementedUnaryCall(getHandleSyncMethod(), responseObserver);
    }
  }

  /**
   * Base class for the server implementation of the service Saga.
   */
  public static abstract class SagaImplBase
      implements io.grpc.BindableService, AsyncService {

    @java.lang.Override public final io.grpc.ServerServiceDefinition bindService() {
      return SagaGrpc.bindService(this);
    }
  }

  /**
   * A stub to allow clients to do asynchronous rpc calls to service Saga.
   */
  public static final class SagaStub
      extends io.grpc.stub.AbstractAsyncStub<SagaStub> {
    private SagaStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected SagaStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new SagaStub(channel, callOptions);
    }

    /**
     */
    public void handle(dev.angzarr.EventBook request,
        io.grpc.stub.StreamObserver<com.google.protobuf.Empty> responseObserver) {
      io.grpc.stub.ClientCalls.asyncUnaryCall(
          getChannel().newCall(getHandleMethod(), getCallOptions()), request, responseObserver);
    }

    /**
     */
    public void handleSync(dev.angzarr.EventBook request,
        io.grpc.stub.StreamObserver<dev.angzarr.SagaResponse> responseObserver) {
      io.grpc.stub.ClientCalls.asyncUnaryCall(
          getChannel().newCall(getHandleSyncMethod(), getCallOptions()), request, responseObserver);
    }
  }

  /**
   * A stub to allow clients to do synchronous rpc calls to service Saga.
   */
  public static final class SagaBlockingStub
      extends io.grpc.stub.AbstractBlockingStub<SagaBlockingStub> {
    private SagaBlockingStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected SagaBlockingStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new SagaBlockingStub(channel, callOptions);
    }

    /**
     */
    public com.google.protobuf.Empty handle(dev.angzarr.EventBook request) {
      return io.grpc.stub.ClientCalls.blockingUnaryCall(
          getChannel(), getHandleMethod(), getCallOptions(), request);
    }

    /**
     */
    public dev.angzarr.SagaResponse handleSync(dev.angzarr.EventBook request) {
      return io.grpc.stub.ClientCalls.blockingUnaryCall(
          getChannel(), getHandleSyncMethod(), getCallOptions(), request);
    }
  }

  /**
   * A stub to allow clients to do ListenableFuture-style rpc calls to service Saga.
   */
  public static final class SagaFutureStub
      extends io.grpc.stub.AbstractFutureStub<SagaFutureStub> {
    private SagaFutureStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected SagaFutureStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new SagaFutureStub(channel, callOptions);
    }

    /**
     */
    public com.google.common.util.concurrent.ListenableFuture<com.google.protobuf.Empty> handle(
        dev.angzarr.EventBook request) {
      return io.grpc.stub.ClientCalls.futureUnaryCall(
          getChannel().newCall(getHandleMethod(), getCallOptions()), request);
    }

    /**
     */
    public com.google.common.util.concurrent.ListenableFuture<dev.angzarr.SagaResponse> handleSync(
        dev.angzarr.EventBook request) {
      return io.grpc.stub.ClientCalls.futureUnaryCall(
          getChannel().newCall(getHandleSyncMethod(), getCallOptions()), request);
    }
  }

  private static final int METHODID_HANDLE = 0;
  private static final int METHODID_HANDLE_SYNC = 1;

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
        case METHODID_HANDLE:
          serviceImpl.handle((dev.angzarr.EventBook) request,
              (io.grpc.stub.StreamObserver<com.google.protobuf.Empty>) responseObserver);
          break;
        case METHODID_HANDLE_SYNC:
          serviceImpl.handleSync((dev.angzarr.EventBook) request,
              (io.grpc.stub.StreamObserver<dev.angzarr.SagaResponse>) responseObserver);
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
          getHandleMethod(),
          io.grpc.stub.ServerCalls.asyncUnaryCall(
            new MethodHandlers<
              dev.angzarr.EventBook,
              com.google.protobuf.Empty>(
                service, METHODID_HANDLE)))
        .addMethod(
          getHandleSyncMethod(),
          io.grpc.stub.ServerCalls.asyncUnaryCall(
            new MethodHandlers<
              dev.angzarr.EventBook,
              dev.angzarr.SagaResponse>(
                service, METHODID_HANDLE_SYNC)))
        .build();
  }

  private static abstract class SagaBaseDescriptorSupplier
      implements io.grpc.protobuf.ProtoFileDescriptorSupplier, io.grpc.protobuf.ProtoServiceDescriptorSupplier {
    SagaBaseDescriptorSupplier() {}

    @java.lang.Override
    public com.google.protobuf.Descriptors.FileDescriptor getFileDescriptor() {
      return dev.angzarr.Angzarr.getDescriptor();
    }

    @java.lang.Override
    public com.google.protobuf.Descriptors.ServiceDescriptor getServiceDescriptor() {
      return getFileDescriptor().findServiceByName("Saga");
    }
  }

  private static final class SagaFileDescriptorSupplier
      extends SagaBaseDescriptorSupplier {
    SagaFileDescriptorSupplier() {}
  }

  private static final class SagaMethodDescriptorSupplier
      extends SagaBaseDescriptorSupplier
      implements io.grpc.protobuf.ProtoMethodDescriptorSupplier {
    private final java.lang.String methodName;

    SagaMethodDescriptorSupplier(java.lang.String methodName) {
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
      synchronized (SagaGrpc.class) {
        result = serviceDescriptor;
        if (result == null) {
          serviceDescriptor = result = io.grpc.ServiceDescriptor.newBuilder(SERVICE_NAME)
              .setSchemaDescriptor(new SagaFileDescriptorSupplier())
              .addMethod(getHandleMethod())
              .addMethod(getHandleSyncMethod())
              .build();
        }
      }
    }
    return result;
  }
}
