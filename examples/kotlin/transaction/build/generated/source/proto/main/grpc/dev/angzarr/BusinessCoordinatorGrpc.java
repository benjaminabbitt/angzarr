package dev.angzarr;

import static io.grpc.MethodDescriptor.generateFullMethodName;

/**
 */
@javax.annotation.Generated(
    value = "by gRPC proto compiler (version 1.60.0)",
    comments = "Source: angzarr/angzarr.proto")
@io.grpc.stub.annotations.GrpcGenerated
public final class BusinessCoordinatorGrpc {

  private BusinessCoordinatorGrpc() {}

  public static final java.lang.String SERVICE_NAME = "angzarr.BusinessCoordinator";

  // Static method descriptors that strictly reflect the proto.
  private static volatile io.grpc.MethodDescriptor<dev.angzarr.CommandBook,
      dev.angzarr.CommandResponse> getHandleMethod;

  @io.grpc.stub.annotations.RpcMethod(
      fullMethodName = SERVICE_NAME + '/' + "Handle",
      requestType = dev.angzarr.CommandBook.class,
      responseType = dev.angzarr.CommandResponse.class,
      methodType = io.grpc.MethodDescriptor.MethodType.UNARY)
  public static io.grpc.MethodDescriptor<dev.angzarr.CommandBook,
      dev.angzarr.CommandResponse> getHandleMethod() {
    io.grpc.MethodDescriptor<dev.angzarr.CommandBook, dev.angzarr.CommandResponse> getHandleMethod;
    if ((getHandleMethod = BusinessCoordinatorGrpc.getHandleMethod) == null) {
      synchronized (BusinessCoordinatorGrpc.class) {
        if ((getHandleMethod = BusinessCoordinatorGrpc.getHandleMethod) == null) {
          BusinessCoordinatorGrpc.getHandleMethod = getHandleMethod =
              io.grpc.MethodDescriptor.<dev.angzarr.CommandBook, dev.angzarr.CommandResponse>newBuilder()
              .setType(io.grpc.MethodDescriptor.MethodType.UNARY)
              .setFullMethodName(generateFullMethodName(SERVICE_NAME, "Handle"))
              .setSampledToLocalTracing(true)
              .setRequestMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.CommandBook.getDefaultInstance()))
              .setResponseMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.CommandResponse.getDefaultInstance()))
              .setSchemaDescriptor(new BusinessCoordinatorMethodDescriptorSupplier("Handle"))
              .build();
        }
      }
    }
    return getHandleMethod;
  }

  private static volatile io.grpc.MethodDescriptor<dev.angzarr.EventBook,
      dev.angzarr.CommandResponse> getRecordMethod;

  @io.grpc.stub.annotations.RpcMethod(
      fullMethodName = SERVICE_NAME + '/' + "Record",
      requestType = dev.angzarr.EventBook.class,
      responseType = dev.angzarr.CommandResponse.class,
      methodType = io.grpc.MethodDescriptor.MethodType.UNARY)
  public static io.grpc.MethodDescriptor<dev.angzarr.EventBook,
      dev.angzarr.CommandResponse> getRecordMethod() {
    io.grpc.MethodDescriptor<dev.angzarr.EventBook, dev.angzarr.CommandResponse> getRecordMethod;
    if ((getRecordMethod = BusinessCoordinatorGrpc.getRecordMethod) == null) {
      synchronized (BusinessCoordinatorGrpc.class) {
        if ((getRecordMethod = BusinessCoordinatorGrpc.getRecordMethod) == null) {
          BusinessCoordinatorGrpc.getRecordMethod = getRecordMethod =
              io.grpc.MethodDescriptor.<dev.angzarr.EventBook, dev.angzarr.CommandResponse>newBuilder()
              .setType(io.grpc.MethodDescriptor.MethodType.UNARY)
              .setFullMethodName(generateFullMethodName(SERVICE_NAME, "Record"))
              .setSampledToLocalTracing(true)
              .setRequestMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.EventBook.getDefaultInstance()))
              .setResponseMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.CommandResponse.getDefaultInstance()))
              .setSchemaDescriptor(new BusinessCoordinatorMethodDescriptorSupplier("Record"))
              .build();
        }
      }
    }
    return getRecordMethod;
  }

  /**
   * Creates a new async stub that supports all call types for the service
   */
  public static BusinessCoordinatorStub newStub(io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<BusinessCoordinatorStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<BusinessCoordinatorStub>() {
        @java.lang.Override
        public BusinessCoordinatorStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new BusinessCoordinatorStub(channel, callOptions);
        }
      };
    return BusinessCoordinatorStub.newStub(factory, channel);
  }

  /**
   * Creates a new blocking-style stub that supports unary and streaming output calls on the service
   */
  public static BusinessCoordinatorBlockingStub newBlockingStub(
      io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<BusinessCoordinatorBlockingStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<BusinessCoordinatorBlockingStub>() {
        @java.lang.Override
        public BusinessCoordinatorBlockingStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new BusinessCoordinatorBlockingStub(channel, callOptions);
        }
      };
    return BusinessCoordinatorBlockingStub.newStub(factory, channel);
  }

  /**
   * Creates a new ListenableFuture-style stub that supports unary calls on the service
   */
  public static BusinessCoordinatorFutureStub newFutureStub(
      io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<BusinessCoordinatorFutureStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<BusinessCoordinatorFutureStub>() {
        @java.lang.Override
        public BusinessCoordinatorFutureStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new BusinessCoordinatorFutureStub(channel, callOptions);
        }
      };
    return BusinessCoordinatorFutureStub.newStub(factory, channel);
  }

  /**
   */
  public interface AsyncService {

    /**
     */
    default void handle(dev.angzarr.CommandBook request,
        io.grpc.stub.StreamObserver<dev.angzarr.CommandResponse> responseObserver) {
      io.grpc.stub.ServerCalls.asyncUnimplementedUnaryCall(getHandleMethod(), responseObserver);
    }

    /**
     */
    default void record(dev.angzarr.EventBook request,
        io.grpc.stub.StreamObserver<dev.angzarr.CommandResponse> responseObserver) {
      io.grpc.stub.ServerCalls.asyncUnimplementedUnaryCall(getRecordMethod(), responseObserver);
    }
  }

  /**
   * Base class for the server implementation of the service BusinessCoordinator.
   */
  public static abstract class BusinessCoordinatorImplBase
      implements io.grpc.BindableService, AsyncService {

    @java.lang.Override public final io.grpc.ServerServiceDefinition bindService() {
      return BusinessCoordinatorGrpc.bindService(this);
    }
  }

  /**
   * A stub to allow clients to do asynchronous rpc calls to service BusinessCoordinator.
   */
  public static final class BusinessCoordinatorStub
      extends io.grpc.stub.AbstractAsyncStub<BusinessCoordinatorStub> {
    private BusinessCoordinatorStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected BusinessCoordinatorStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new BusinessCoordinatorStub(channel, callOptions);
    }

    /**
     */
    public void handle(dev.angzarr.CommandBook request,
        io.grpc.stub.StreamObserver<dev.angzarr.CommandResponse> responseObserver) {
      io.grpc.stub.ClientCalls.asyncUnaryCall(
          getChannel().newCall(getHandleMethod(), getCallOptions()), request, responseObserver);
    }

    /**
     */
    public void record(dev.angzarr.EventBook request,
        io.grpc.stub.StreamObserver<dev.angzarr.CommandResponse> responseObserver) {
      io.grpc.stub.ClientCalls.asyncUnaryCall(
          getChannel().newCall(getRecordMethod(), getCallOptions()), request, responseObserver);
    }
  }

  /**
   * A stub to allow clients to do synchronous rpc calls to service BusinessCoordinator.
   */
  public static final class BusinessCoordinatorBlockingStub
      extends io.grpc.stub.AbstractBlockingStub<BusinessCoordinatorBlockingStub> {
    private BusinessCoordinatorBlockingStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected BusinessCoordinatorBlockingStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new BusinessCoordinatorBlockingStub(channel, callOptions);
    }

    /**
     */
    public dev.angzarr.CommandResponse handle(dev.angzarr.CommandBook request) {
      return io.grpc.stub.ClientCalls.blockingUnaryCall(
          getChannel(), getHandleMethod(), getCallOptions(), request);
    }

    /**
     */
    public dev.angzarr.CommandResponse record(dev.angzarr.EventBook request) {
      return io.grpc.stub.ClientCalls.blockingUnaryCall(
          getChannel(), getRecordMethod(), getCallOptions(), request);
    }
  }

  /**
   * A stub to allow clients to do ListenableFuture-style rpc calls to service BusinessCoordinator.
   */
  public static final class BusinessCoordinatorFutureStub
      extends io.grpc.stub.AbstractFutureStub<BusinessCoordinatorFutureStub> {
    private BusinessCoordinatorFutureStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected BusinessCoordinatorFutureStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new BusinessCoordinatorFutureStub(channel, callOptions);
    }

    /**
     */
    public com.google.common.util.concurrent.ListenableFuture<dev.angzarr.CommandResponse> handle(
        dev.angzarr.CommandBook request) {
      return io.grpc.stub.ClientCalls.futureUnaryCall(
          getChannel().newCall(getHandleMethod(), getCallOptions()), request);
    }

    /**
     */
    public com.google.common.util.concurrent.ListenableFuture<dev.angzarr.CommandResponse> record(
        dev.angzarr.EventBook request) {
      return io.grpc.stub.ClientCalls.futureUnaryCall(
          getChannel().newCall(getRecordMethod(), getCallOptions()), request);
    }
  }

  private static final int METHODID_HANDLE = 0;
  private static final int METHODID_RECORD = 1;

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
          serviceImpl.handle((dev.angzarr.CommandBook) request,
              (io.grpc.stub.StreamObserver<dev.angzarr.CommandResponse>) responseObserver);
          break;
        case METHODID_RECORD:
          serviceImpl.record((dev.angzarr.EventBook) request,
              (io.grpc.stub.StreamObserver<dev.angzarr.CommandResponse>) responseObserver);
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
              dev.angzarr.CommandBook,
              dev.angzarr.CommandResponse>(
                service, METHODID_HANDLE)))
        .addMethod(
          getRecordMethod(),
          io.grpc.stub.ServerCalls.asyncUnaryCall(
            new MethodHandlers<
              dev.angzarr.EventBook,
              dev.angzarr.CommandResponse>(
                service, METHODID_RECORD)))
        .build();
  }

  private static abstract class BusinessCoordinatorBaseDescriptorSupplier
      implements io.grpc.protobuf.ProtoFileDescriptorSupplier, io.grpc.protobuf.ProtoServiceDescriptorSupplier {
    BusinessCoordinatorBaseDescriptorSupplier() {}

    @java.lang.Override
    public com.google.protobuf.Descriptors.FileDescriptor getFileDescriptor() {
      return dev.angzarr.Angzarr.getDescriptor();
    }

    @java.lang.Override
    public com.google.protobuf.Descriptors.ServiceDescriptor getServiceDescriptor() {
      return getFileDescriptor().findServiceByName("BusinessCoordinator");
    }
  }

  private static final class BusinessCoordinatorFileDescriptorSupplier
      extends BusinessCoordinatorBaseDescriptorSupplier {
    BusinessCoordinatorFileDescriptorSupplier() {}
  }

  private static final class BusinessCoordinatorMethodDescriptorSupplier
      extends BusinessCoordinatorBaseDescriptorSupplier
      implements io.grpc.protobuf.ProtoMethodDescriptorSupplier {
    private final java.lang.String methodName;

    BusinessCoordinatorMethodDescriptorSupplier(java.lang.String methodName) {
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
      synchronized (BusinessCoordinatorGrpc.class) {
        result = serviceDescriptor;
        if (result == null) {
          serviceDescriptor = result = io.grpc.ServiceDescriptor.newBuilder(SERVICE_NAME)
              .setSchemaDescriptor(new BusinessCoordinatorFileDescriptorSupplier())
              .addMethod(getHandleMethod())
              .addMethod(getRecordMethod())
              .build();
        }
      }
    }
    return result;
  }
}
