package dev.angzarr;

import static io.grpc.MethodDescriptor.generateFullMethodName;

/**
 */
@javax.annotation.Generated(
    value = "by gRPC proto compiler (version 1.60.0)",
    comments = "Source: angzarr/angzarr.proto")
@io.grpc.stub.annotations.GrpcGenerated
public final class ProjectorCoordinatorGrpc {

  private ProjectorCoordinatorGrpc() {}

  public static final java.lang.String SERVICE_NAME = "angzarr.ProjectorCoordinator";

  // Static method descriptors that strictly reflect the proto.
  private static volatile io.grpc.MethodDescriptor<dev.angzarr.EventBook,
      dev.angzarr.Projection> getHandleSyncMethod;

  @io.grpc.stub.annotations.RpcMethod(
      fullMethodName = SERVICE_NAME + '/' + "HandleSync",
      requestType = dev.angzarr.EventBook.class,
      responseType = dev.angzarr.Projection.class,
      methodType = io.grpc.MethodDescriptor.MethodType.UNARY)
  public static io.grpc.MethodDescriptor<dev.angzarr.EventBook,
      dev.angzarr.Projection> getHandleSyncMethod() {
    io.grpc.MethodDescriptor<dev.angzarr.EventBook, dev.angzarr.Projection> getHandleSyncMethod;
    if ((getHandleSyncMethod = ProjectorCoordinatorGrpc.getHandleSyncMethod) == null) {
      synchronized (ProjectorCoordinatorGrpc.class) {
        if ((getHandleSyncMethod = ProjectorCoordinatorGrpc.getHandleSyncMethod) == null) {
          ProjectorCoordinatorGrpc.getHandleSyncMethod = getHandleSyncMethod =
              io.grpc.MethodDescriptor.<dev.angzarr.EventBook, dev.angzarr.Projection>newBuilder()
              .setType(io.grpc.MethodDescriptor.MethodType.UNARY)
              .setFullMethodName(generateFullMethodName(SERVICE_NAME, "HandleSync"))
              .setSampledToLocalTracing(true)
              .setRequestMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.EventBook.getDefaultInstance()))
              .setResponseMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.Projection.getDefaultInstance()))
              .setSchemaDescriptor(new ProjectorCoordinatorMethodDescriptorSupplier("HandleSync"))
              .build();
        }
      }
    }
    return getHandleSyncMethod;
  }

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
    if ((getHandleMethod = ProjectorCoordinatorGrpc.getHandleMethod) == null) {
      synchronized (ProjectorCoordinatorGrpc.class) {
        if ((getHandleMethod = ProjectorCoordinatorGrpc.getHandleMethod) == null) {
          ProjectorCoordinatorGrpc.getHandleMethod = getHandleMethod =
              io.grpc.MethodDescriptor.<dev.angzarr.EventBook, com.google.protobuf.Empty>newBuilder()
              .setType(io.grpc.MethodDescriptor.MethodType.UNARY)
              .setFullMethodName(generateFullMethodName(SERVICE_NAME, "Handle"))
              .setSampledToLocalTracing(true)
              .setRequestMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.EventBook.getDefaultInstance()))
              .setResponseMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  com.google.protobuf.Empty.getDefaultInstance()))
              .setSchemaDescriptor(new ProjectorCoordinatorMethodDescriptorSupplier("Handle"))
              .build();
        }
      }
    }
    return getHandleMethod;
  }

  /**
   * Creates a new async stub that supports all call types for the service
   */
  public static ProjectorCoordinatorStub newStub(io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<ProjectorCoordinatorStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<ProjectorCoordinatorStub>() {
        @java.lang.Override
        public ProjectorCoordinatorStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new ProjectorCoordinatorStub(channel, callOptions);
        }
      };
    return ProjectorCoordinatorStub.newStub(factory, channel);
  }

  /**
   * Creates a new blocking-style stub that supports unary and streaming output calls on the service
   */
  public static ProjectorCoordinatorBlockingStub newBlockingStub(
      io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<ProjectorCoordinatorBlockingStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<ProjectorCoordinatorBlockingStub>() {
        @java.lang.Override
        public ProjectorCoordinatorBlockingStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new ProjectorCoordinatorBlockingStub(channel, callOptions);
        }
      };
    return ProjectorCoordinatorBlockingStub.newStub(factory, channel);
  }

  /**
   * Creates a new ListenableFuture-style stub that supports unary calls on the service
   */
  public static ProjectorCoordinatorFutureStub newFutureStub(
      io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<ProjectorCoordinatorFutureStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<ProjectorCoordinatorFutureStub>() {
        @java.lang.Override
        public ProjectorCoordinatorFutureStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new ProjectorCoordinatorFutureStub(channel, callOptions);
        }
      };
    return ProjectorCoordinatorFutureStub.newStub(factory, channel);
  }

  /**
   */
  public interface AsyncService {

    /**
     */
    default void handleSync(dev.angzarr.EventBook request,
        io.grpc.stub.StreamObserver<dev.angzarr.Projection> responseObserver) {
      io.grpc.stub.ServerCalls.asyncUnimplementedUnaryCall(getHandleSyncMethod(), responseObserver);
    }

    /**
     */
    default void handle(dev.angzarr.EventBook request,
        io.grpc.stub.StreamObserver<com.google.protobuf.Empty> responseObserver) {
      io.grpc.stub.ServerCalls.asyncUnimplementedUnaryCall(getHandleMethod(), responseObserver);
    }
  }

  /**
   * Base class for the server implementation of the service ProjectorCoordinator.
   */
  public static abstract class ProjectorCoordinatorImplBase
      implements io.grpc.BindableService, AsyncService {

    @java.lang.Override public final io.grpc.ServerServiceDefinition bindService() {
      return ProjectorCoordinatorGrpc.bindService(this);
    }
  }

  /**
   * A stub to allow clients to do asynchronous rpc calls to service ProjectorCoordinator.
   */
  public static final class ProjectorCoordinatorStub
      extends io.grpc.stub.AbstractAsyncStub<ProjectorCoordinatorStub> {
    private ProjectorCoordinatorStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected ProjectorCoordinatorStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new ProjectorCoordinatorStub(channel, callOptions);
    }

    /**
     */
    public void handleSync(dev.angzarr.EventBook request,
        io.grpc.stub.StreamObserver<dev.angzarr.Projection> responseObserver) {
      io.grpc.stub.ClientCalls.asyncUnaryCall(
          getChannel().newCall(getHandleSyncMethod(), getCallOptions()), request, responseObserver);
    }

    /**
     */
    public void handle(dev.angzarr.EventBook request,
        io.grpc.stub.StreamObserver<com.google.protobuf.Empty> responseObserver) {
      io.grpc.stub.ClientCalls.asyncUnaryCall(
          getChannel().newCall(getHandleMethod(), getCallOptions()), request, responseObserver);
    }
  }

  /**
   * A stub to allow clients to do synchronous rpc calls to service ProjectorCoordinator.
   */
  public static final class ProjectorCoordinatorBlockingStub
      extends io.grpc.stub.AbstractBlockingStub<ProjectorCoordinatorBlockingStub> {
    private ProjectorCoordinatorBlockingStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected ProjectorCoordinatorBlockingStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new ProjectorCoordinatorBlockingStub(channel, callOptions);
    }

    /**
     */
    public dev.angzarr.Projection handleSync(dev.angzarr.EventBook request) {
      return io.grpc.stub.ClientCalls.blockingUnaryCall(
          getChannel(), getHandleSyncMethod(), getCallOptions(), request);
    }

    /**
     */
    public com.google.protobuf.Empty handle(dev.angzarr.EventBook request) {
      return io.grpc.stub.ClientCalls.blockingUnaryCall(
          getChannel(), getHandleMethod(), getCallOptions(), request);
    }
  }

  /**
   * A stub to allow clients to do ListenableFuture-style rpc calls to service ProjectorCoordinator.
   */
  public static final class ProjectorCoordinatorFutureStub
      extends io.grpc.stub.AbstractFutureStub<ProjectorCoordinatorFutureStub> {
    private ProjectorCoordinatorFutureStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected ProjectorCoordinatorFutureStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new ProjectorCoordinatorFutureStub(channel, callOptions);
    }

    /**
     */
    public com.google.common.util.concurrent.ListenableFuture<dev.angzarr.Projection> handleSync(
        dev.angzarr.EventBook request) {
      return io.grpc.stub.ClientCalls.futureUnaryCall(
          getChannel().newCall(getHandleSyncMethod(), getCallOptions()), request);
    }

    /**
     */
    public com.google.common.util.concurrent.ListenableFuture<com.google.protobuf.Empty> handle(
        dev.angzarr.EventBook request) {
      return io.grpc.stub.ClientCalls.futureUnaryCall(
          getChannel().newCall(getHandleMethod(), getCallOptions()), request);
    }
  }

  private static final int METHODID_HANDLE_SYNC = 0;
  private static final int METHODID_HANDLE = 1;

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
        case METHODID_HANDLE_SYNC:
          serviceImpl.handleSync((dev.angzarr.EventBook) request,
              (io.grpc.stub.StreamObserver<dev.angzarr.Projection>) responseObserver);
          break;
        case METHODID_HANDLE:
          serviceImpl.handle((dev.angzarr.EventBook) request,
              (io.grpc.stub.StreamObserver<com.google.protobuf.Empty>) responseObserver);
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
          getHandleSyncMethod(),
          io.grpc.stub.ServerCalls.asyncUnaryCall(
            new MethodHandlers<
              dev.angzarr.EventBook,
              dev.angzarr.Projection>(
                service, METHODID_HANDLE_SYNC)))
        .addMethod(
          getHandleMethod(),
          io.grpc.stub.ServerCalls.asyncUnaryCall(
            new MethodHandlers<
              dev.angzarr.EventBook,
              com.google.protobuf.Empty>(
                service, METHODID_HANDLE)))
        .build();
  }

  private static abstract class ProjectorCoordinatorBaseDescriptorSupplier
      implements io.grpc.protobuf.ProtoFileDescriptorSupplier, io.grpc.protobuf.ProtoServiceDescriptorSupplier {
    ProjectorCoordinatorBaseDescriptorSupplier() {}

    @java.lang.Override
    public com.google.protobuf.Descriptors.FileDescriptor getFileDescriptor() {
      return dev.angzarr.Angzarr.getDescriptor();
    }

    @java.lang.Override
    public com.google.protobuf.Descriptors.ServiceDescriptor getServiceDescriptor() {
      return getFileDescriptor().findServiceByName("ProjectorCoordinator");
    }
  }

  private static final class ProjectorCoordinatorFileDescriptorSupplier
      extends ProjectorCoordinatorBaseDescriptorSupplier {
    ProjectorCoordinatorFileDescriptorSupplier() {}
  }

  private static final class ProjectorCoordinatorMethodDescriptorSupplier
      extends ProjectorCoordinatorBaseDescriptorSupplier
      implements io.grpc.protobuf.ProtoMethodDescriptorSupplier {
    private final java.lang.String methodName;

    ProjectorCoordinatorMethodDescriptorSupplier(java.lang.String methodName) {
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
      synchronized (ProjectorCoordinatorGrpc.class) {
        result = serviceDescriptor;
        if (result == null) {
          serviceDescriptor = result = io.grpc.ServiceDescriptor.newBuilder(SERVICE_NAME)
              .setSchemaDescriptor(new ProjectorCoordinatorFileDescriptorSupplier())
              .addMethod(getHandleSyncMethod())
              .addMethod(getHandleMethod())
              .build();
        }
      }
    }
    return result;
  }
}
