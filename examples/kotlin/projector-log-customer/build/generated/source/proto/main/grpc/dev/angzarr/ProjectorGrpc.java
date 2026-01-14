package dev.angzarr;

import static io.grpc.MethodDescriptor.generateFullMethodName;

/**
 */
@javax.annotation.Generated(
    value = "by gRPC proto compiler (version 1.60.0)",
    comments = "Source: angzarr/angzarr.proto")
@io.grpc.stub.annotations.GrpcGenerated
public final class ProjectorGrpc {

  private ProjectorGrpc() {}

  public static final java.lang.String SERVICE_NAME = "angzarr.Projector";

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
    if ((getHandleMethod = ProjectorGrpc.getHandleMethod) == null) {
      synchronized (ProjectorGrpc.class) {
        if ((getHandleMethod = ProjectorGrpc.getHandleMethod) == null) {
          ProjectorGrpc.getHandleMethod = getHandleMethod =
              io.grpc.MethodDescriptor.<dev.angzarr.EventBook, com.google.protobuf.Empty>newBuilder()
              .setType(io.grpc.MethodDescriptor.MethodType.UNARY)
              .setFullMethodName(generateFullMethodName(SERVICE_NAME, "Handle"))
              .setSampledToLocalTracing(true)
              .setRequestMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.EventBook.getDefaultInstance()))
              .setResponseMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  com.google.protobuf.Empty.getDefaultInstance()))
              .setSchemaDescriptor(new ProjectorMethodDescriptorSupplier("Handle"))
              .build();
        }
      }
    }
    return getHandleMethod;
  }

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
    if ((getHandleSyncMethod = ProjectorGrpc.getHandleSyncMethod) == null) {
      synchronized (ProjectorGrpc.class) {
        if ((getHandleSyncMethod = ProjectorGrpc.getHandleSyncMethod) == null) {
          ProjectorGrpc.getHandleSyncMethod = getHandleSyncMethod =
              io.grpc.MethodDescriptor.<dev.angzarr.EventBook, dev.angzarr.Projection>newBuilder()
              .setType(io.grpc.MethodDescriptor.MethodType.UNARY)
              .setFullMethodName(generateFullMethodName(SERVICE_NAME, "HandleSync"))
              .setSampledToLocalTracing(true)
              .setRequestMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.EventBook.getDefaultInstance()))
              .setResponseMarshaller(io.grpc.protobuf.ProtoUtils.marshaller(
                  dev.angzarr.Projection.getDefaultInstance()))
              .setSchemaDescriptor(new ProjectorMethodDescriptorSupplier("HandleSync"))
              .build();
        }
      }
    }
    return getHandleSyncMethod;
  }

  /**
   * Creates a new async stub that supports all call types for the service
   */
  public static ProjectorStub newStub(io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<ProjectorStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<ProjectorStub>() {
        @java.lang.Override
        public ProjectorStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new ProjectorStub(channel, callOptions);
        }
      };
    return ProjectorStub.newStub(factory, channel);
  }

  /**
   * Creates a new blocking-style stub that supports unary and streaming output calls on the service
   */
  public static ProjectorBlockingStub newBlockingStub(
      io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<ProjectorBlockingStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<ProjectorBlockingStub>() {
        @java.lang.Override
        public ProjectorBlockingStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new ProjectorBlockingStub(channel, callOptions);
        }
      };
    return ProjectorBlockingStub.newStub(factory, channel);
  }

  /**
   * Creates a new ListenableFuture-style stub that supports unary calls on the service
   */
  public static ProjectorFutureStub newFutureStub(
      io.grpc.Channel channel) {
    io.grpc.stub.AbstractStub.StubFactory<ProjectorFutureStub> factory =
      new io.grpc.stub.AbstractStub.StubFactory<ProjectorFutureStub>() {
        @java.lang.Override
        public ProjectorFutureStub newStub(io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
          return new ProjectorFutureStub(channel, callOptions);
        }
      };
    return ProjectorFutureStub.newStub(factory, channel);
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
        io.grpc.stub.StreamObserver<dev.angzarr.Projection> responseObserver) {
      io.grpc.stub.ServerCalls.asyncUnimplementedUnaryCall(getHandleSyncMethod(), responseObserver);
    }
  }

  /**
   * Base class for the server implementation of the service Projector.
   */
  public static abstract class ProjectorImplBase
      implements io.grpc.BindableService, AsyncService {

    @java.lang.Override public final io.grpc.ServerServiceDefinition bindService() {
      return ProjectorGrpc.bindService(this);
    }
  }

  /**
   * A stub to allow clients to do asynchronous rpc calls to service Projector.
   */
  public static final class ProjectorStub
      extends io.grpc.stub.AbstractAsyncStub<ProjectorStub> {
    private ProjectorStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected ProjectorStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new ProjectorStub(channel, callOptions);
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
        io.grpc.stub.StreamObserver<dev.angzarr.Projection> responseObserver) {
      io.grpc.stub.ClientCalls.asyncUnaryCall(
          getChannel().newCall(getHandleSyncMethod(), getCallOptions()), request, responseObserver);
    }
  }

  /**
   * A stub to allow clients to do synchronous rpc calls to service Projector.
   */
  public static final class ProjectorBlockingStub
      extends io.grpc.stub.AbstractBlockingStub<ProjectorBlockingStub> {
    private ProjectorBlockingStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected ProjectorBlockingStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new ProjectorBlockingStub(channel, callOptions);
    }

    /**
     */
    public com.google.protobuf.Empty handle(dev.angzarr.EventBook request) {
      return io.grpc.stub.ClientCalls.blockingUnaryCall(
          getChannel(), getHandleMethod(), getCallOptions(), request);
    }

    /**
     */
    public dev.angzarr.Projection handleSync(dev.angzarr.EventBook request) {
      return io.grpc.stub.ClientCalls.blockingUnaryCall(
          getChannel(), getHandleSyncMethod(), getCallOptions(), request);
    }
  }

  /**
   * A stub to allow clients to do ListenableFuture-style rpc calls to service Projector.
   */
  public static final class ProjectorFutureStub
      extends io.grpc.stub.AbstractFutureStub<ProjectorFutureStub> {
    private ProjectorFutureStub(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      super(channel, callOptions);
    }

    @java.lang.Override
    protected ProjectorFutureStub build(
        io.grpc.Channel channel, io.grpc.CallOptions callOptions) {
      return new ProjectorFutureStub(channel, callOptions);
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
    public com.google.common.util.concurrent.ListenableFuture<dev.angzarr.Projection> handleSync(
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
              (io.grpc.stub.StreamObserver<dev.angzarr.Projection>) responseObserver);
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
              dev.angzarr.Projection>(
                service, METHODID_HANDLE_SYNC)))
        .build();
  }

  private static abstract class ProjectorBaseDescriptorSupplier
      implements io.grpc.protobuf.ProtoFileDescriptorSupplier, io.grpc.protobuf.ProtoServiceDescriptorSupplier {
    ProjectorBaseDescriptorSupplier() {}

    @java.lang.Override
    public com.google.protobuf.Descriptors.FileDescriptor getFileDescriptor() {
      return dev.angzarr.Angzarr.getDescriptor();
    }

    @java.lang.Override
    public com.google.protobuf.Descriptors.ServiceDescriptor getServiceDescriptor() {
      return getFileDescriptor().findServiceByName("Projector");
    }
  }

  private static final class ProjectorFileDescriptorSupplier
      extends ProjectorBaseDescriptorSupplier {
    ProjectorFileDescriptorSupplier() {}
  }

  private static final class ProjectorMethodDescriptorSupplier
      extends ProjectorBaseDescriptorSupplier
      implements io.grpc.protobuf.ProtoMethodDescriptorSupplier {
    private final java.lang.String methodName;

    ProjectorMethodDescriptorSupplier(java.lang.String methodName) {
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
      synchronized (ProjectorGrpc.class) {
        result = serviceDescriptor;
        if (result == null) {
          serviceDescriptor = result = io.grpc.ServiceDescriptor.newBuilder(SERVICE_NAME)
              .setSchemaDescriptor(new ProjectorFileDescriptorSupplier())
              .addMethod(getHandleMethod())
              .addMethod(getHandleSyncMethod())
              .build();
        }
      }
    }
    return result;
  }
}
