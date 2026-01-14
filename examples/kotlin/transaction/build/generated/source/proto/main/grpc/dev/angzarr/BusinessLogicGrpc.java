package dev.angzarr;

import static io.grpc.MethodDescriptor.generateFullMethodName;

/**
 */
@javax.annotation.Generated(
    value = "by gRPC proto compiler (version 1.60.0)",
    comments = "Source: angzarr/angzarr.proto")
@io.grpc.stub.annotations.GrpcGenerated
public final class BusinessLogicGrpc {

  private BusinessLogicGrpc() {}

  public static final java.lang.String SERVICE_NAME = "angzarr.BusinessLogic";

  // Static method descriptors that strictly reflect the proto.
  private static volatile io.grpc.MethodDescriptor<dev.angzarr.ContextualCommand,
      dev.angzarr.BusinessResponse> getHandleMethod;

  @io.grpc.stub.annotations.RpcMethod(
      fullMethodName = SERVICE_NAME + '/' + "Handle",
      requestType = dev.angzarr.ContextualCommand.class,
      responseType = dev.angzarr.BusinessResponse.class,
      methodType = io.grpc.MethodDescriptor.MethodType.UNARY)
  public static io.grpc.MethodDescriptor<dev.angzarr.ContextualCommand,
      dev.angzarr.BusinessResponse> getHandleMethod() {
    io.grpc.MethodDescriptor<dev.angzarr.ContextualCommand, dev.angzarr.BusinessResponse> getHandleMethod;
    if ((getHandleMethod = BusinessLogicGrpc.getHandleMethod) == null) {
      synchronized (BusinessLogicGrpc.class) {
        if ((getHandleMethod = BusinessLogicGrpc.getHandleMethod) == null) {
          BusinessLogicGrpc.getHandleMethod = getHandleMethod =
              io.grpc.MethodDescriptor.<dev.angzarr.ContextualCommand, dev.angzarr.BusinessResponse>newBuilder()
              .setType(io.grpc.MethodDescriptor.MethodType.UNARY)
              .setFullMethodName(generateFullMethodName(SERVICE_NAME, "Handle"))
              .setSampledToLocalTracing(true)
              .setRequestMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.ContextualCommand.getDefaultInstance()))
              .setResponseMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.BusinessResponse.getDefaultInstance()))
              .setSchemaDescriptor(new BusinessLogicMethodDescriptorSupplier("Handle"))
              .build();
        }
      }
    }
    return getHandleMethod;
  }

  /**
   * Creates a new async stub that supports all call types for the service
   */
  public static BusinessLogicStub newStub(io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<BusinessLogicStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<BusinessLogicStub>() {
        @java.lang.Override
        public BusinessLogicStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new BusinessLogicStub(channel, callOptions);
        }
      };
    return BusinessLogicStub.newStub(factory, channel);
  }

  /**
   * Creates a new blocking-style stub that supports unary and streaming output calls on the service
   */
  public static BusinessLogicBlockingStub newBlockingStub(
      io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<BusinessLogicBlockingStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<BusinessLogicBlockingStub>() {
        @java.lang.Override
        public BusinessLogicBlockingStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new BusinessLogicBlockingStub(channel, callOptions);
        }
      };
    return BusinessLogicBlockingStub.newStub(factory, channel);
  }

  /**
   * Creates a new ListenableFuture-style stub that supports unary calls on the service
   */
  public static BusinessLogicFutureStub newFutureStub(
      io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<BusinessLogicFutureStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<BusinessLogicFutureStub>() {
        @java.lang.Override
        public BusinessLogicFutureStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new BusinessLogicFutureStub(channel, callOptions);
        }
      };
    return BusinessLogicFutureStub.newStub(factory, channel);
  }

  /**
   */
  public interface AsyncService {

    /**
     */
    default void handle(dev.angzarr.ContextualCommand request,
        io.grpc.stub.StreamObserver<dev.angzarr.BusinessResponse> responseObserver) {
      io.grpc.stub.ServerCalls.asyncUnimplementedUnaryCall(getHandleMethod(), responseObserver);
    }
  }

  /**
   * Base class for the server implementation of the service BusinessLogic.
   */
  public static abstract class BusinessLogicImplBase
      implements io.grpc.BindableService, AsyncService {

    @java.lang.Override public final io.grpc.ServerServiceDefinition bindService() {
      return BusinessLogicGrpc.bindService(this);
    }
  }

  /**
   * A stub to allow clients to do asynchronous rpc calls to service BusinessLogic.
   */
  public static final class BusinessLogicStub
      extends io.grpc.stub.AbstractAsyncStub<BusinessLogicStub> {
    private BusinessLogicStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected BusinessLogicStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new BusinessLogicStub(channel, callOptions);
    }

    /**
     */
    public void handle(dev.angzarr.ContextualCommand request,
        io.grpc.stub.StreamObserver<dev.angzarr.BusinessResponse> responseObserver) {
      io.grpc.stub.ClientCalls.asyncUnaryCall(
          getChannel().newCall(getHandleMethod(), getCallOptions()), request, responseObserver);
    }
  }

  /**
   * A stub to allow clients to do synchronous rpc calls to service BusinessLogic.
   */
  public static final class BusinessLogicBlockingStub
      extends io.grpc.stub.AbstractBlockingStub<BusinessLogicBlockingStub> {
    private BusinessLogicBlockingStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected BusinessLogicBlockingStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new BusinessLogicBlockingStub(channel, callOptions);
    }

    /**
     */
    public dev.angzarr.BusinessResponse handle(dev.angzarr.ContextualCommand request) {
      return io.grpc.stub.ClientCalls.blockingUnaryCall(
          getChannel(), getHandleMethod(), getCallOptions(), request);
    }
  }

  /**
   * A stub to allow clients to do ListenableFuture-style rpc calls to service BusinessLogic.
   */
  public static final class BusinessLogicFutureStub
      extends io.grpc.stub.AbstractFutureStub<BusinessLogicFutureStub> {
    private BusinessLogicFutureStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected BusinessLogicFutureStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new BusinessLogicFutureStub(channel, callOptions);
    }

    /**
     */
    public com.google.common.util.concurrent.ListenableFuture<dev.angzarr.BusinessResponse> handle(
        dev.angzarr.ContextualCommand request) {
      return io.grpc.stub.ClientCalls.futureUnaryCall(
          getChannel().newCall(getHandleMethod(), getCallOptions()), request);
    }
  }

  private static final int METHODID_HANDLE = 0;

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
          serviceImpl.handle((dev.angzarr.ContextualCommand) request,
              (io.grpc.stub.StreamObserver<dev.angzarr.BusinessResponse>) responseObserver);
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
              dev.angzarr.ContextualCommand,
              dev.angzarr.BusinessResponse>(
                service, METHODID_HANDLE)))
        .build();
  }

  private static abstract class BusinessLogicBaseDescriptorSupplier
      implements io.grpc.protobuf.ProtoFileDescriptorSupplier, io.grpc.protobuf.ProtoServiceDescriptorSupplier {
    BusinessLogicBaseDescriptorSupplier() {}

    @java.lang.Override
    public com.google.protobuf.Descriptors.FileDescriptor getFileDescriptor() {
      return dev.angzarr.Angzarr.getDescriptor();
    }

    @java.lang.Override
    public com.google.protobuf.Descriptors.ServiceDescriptor getServiceDescriptor() {
      return getFileDescriptor().findServiceByName("BusinessLogic");
    }
  }

  private static final class BusinessLogicFileDescriptorSupplier
      extends BusinessLogicBaseDescriptorSupplier {
    BusinessLogicFileDescriptorSupplier() {}
  }

  private static final class BusinessLogicMethodDescriptorSupplier
      extends BusinessLogicBaseDescriptorSupplier
      implements io.grpc.protobuf.ProtoMethodDescriptorSupplier {
    private final java.lang.String methodName;

    BusinessLogicMethodDescriptorSupplier(java.lang.String methodName) {
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
      synchronized (BusinessLogicGrpc.class) {
        result = serviceDescriptor;
        if (result == null) {
          serviceDescriptor = result = io.grpc.ServiceDescriptor.newBuilder(SERVICE_NAME)
              .setSchemaDescriptor(new BusinessLogicFileDescriptorSupplier())
              .addMethod(getHandleMethod())
              .build();
        }
      }
    }
    return result;
  }
}
