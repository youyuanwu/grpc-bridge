```ps1
$env:GRPC_EXPERIMENTS="event_engine_client,-event_engine_listener"

$env:GRPC_TRACE="event_engine,event_engine_endpoint"

$env:GRPC_VERBOSITY="debug"

$env:GRPC_DNS_RESOLVER="native"


python tools/run_tests/run_tests.py -l c++ --compiler cmake_vs2022 -c dbg
```

Potential deadlock in experiment "event_engine_client"
In:
```cpp
EventEngine::ConnectionHandle handle = engine_ptr->Connect(
    [on_connect,
     endpoint](absl::StatusOr<std::unique_ptr<EventEngine::Endpoint>> ep) {
      grpc_core::ApplicationCallbackExecCtx app_ctx;
      grpc_core::ExecCtx exec_ctx;
      absl::Status conn_status = ep.ok() ? absl::OkStatus() : ep.status();
      if (ep.ok()) {
        *endpoint = grpc_event_engine_endpoint_create(std::move(*ep));
      } else {
        *endpoint = nullptr;
      }
      GRPC_EVENT_ENGINE_TRACE("EventEngine::Connect Status: %s",
                              ep.status().ToString().c_str());
      grpc_core::ExecCtx::Run(DEBUG_LOCATION, on_connect,
                              absl_status_to_grpc_error(conn_status));
    },
```
ExecCtx has some deferred task to run on this thread tries handshaker mgr lock, while WindowsEventEngine::Connect's
Run takes conn state lock already.
Usually handshake lock is taken first then the conn state lock.
```cpp
  if (!success) {
    int last_error = WSAGetLastError();
    if (last_error != ERROR_IO_PENDING) {
      if (!Cancel(connection_state->timer_handle)) {
        return EventEngine::ConnectionHandle::kInvalid;
      }
      connection_state->socket->Shutdown(DEBUG_LOCATION, "ConnectEx");
      Run([connection_state = std::move(connection_state),
           status = GRPC_WSA_ERROR(WSAGetLastError(), "ConnectEx")]() mutable {
        grpc_core::MutexLock lock(&connection_state->mu);
        connection_state->on_connected_user_callback(status);
      });
      return EventEngine::ConnectionHandle::kInvalid;
    }
  }
```

```
[ RUN      ] RetryHttp2Test.ConnectivityWatch/Chttp2FullstackLocalUds
E0211 14:26:48.382000000  6692 cq_verifier.cc:425] Verify empty completion queue for 100ms
E0211 14:26:48.493000000  6692 cq_verifier.cc:367] Verify tag(1)-Γ¥î for 60000ms
E0211 14:26:49.024000000  6692 cq_verifier.cc:367] Verify tag(2)-Γ£à for 10000ms
[mutex.cc : 1419] RAW: Potential Mutex deadlock: (handshake)
        @ 010E39C9 absl::lts_20240116::DebugOnlyDeadlockCheck
        @ 010DDFA4 absl::lts_20240116::Mutex::Lock
        @ 007872A0 absl::lts_20240116::MutexLock::MutexLock
        @ 00A053EB grpc_core::HandshakeManager::CallNextHandshakerFn
        @ 009D6B14 exec_ctx_run
        @ 009D6600 grpc_core::ExecCtx::Flush
        @ 0078B9DD grpc_core::ExecCtx::~ExecCtx
        @ 00BF4B01 <lambda_41920641b739d4c7bb98a916290a31a8>::operator()
        @ 00BF505B absl::lts_20240116::base_internal::Callable::Invoke<<lambda_41920641b739d4c7bb98a916290a31a8> &,absl::lts_20240116::StatusOr<std::unique_ptr<grpc_event_engine::experimental::EventEngine::Endpoint,...
        @ 00BF5162 absl::lts_20240116::base_internal::invoke<<lambda_41920641b739d4c7bb98a916290a31a8> &,absl::lts_20240116::StatusOr<std::unique_ptr<grpc_event_engine::experimental::EventEngine::Endpoint,std::defau...
        @ 00BF50A2 absl::lts_20240116::internal_any_invocable::InvokeR<void,<lambda_41920641b739d4c7bb98a916290a31a8> &,absl::lts_20240116::StatusOr<std::unique_ptr<grpc_event_engine::experimental::EventEngine::Endp...
        @ 00BF50E7 absl::lts_20240116::internal_any_invocable::LocalInvoker<0,void,<lambda_41920641b739d4c7bb98a916290a31a8> &,absl::lts_20240116::StatusOr<std::unique_ptr<grpc_event_engine::experimental::EventEngin...
        @ 007B0B33 absl::lts_20240116::internal_any_invocable::Impl<void __cdecl(absl::lts_20240116::StatusOr<std::unique_ptr<grpc_event_engine::experimental::EventEngine::Endpoint,std::default_delete<grpc_event_eng...
        @ 00B94BDA <lambda_d34c13f9b2fef67565400b6feddab80d>::operator()
        @ 00B99DA6 absl::lts_20240116::base_internal::Callable::Invoke<<lambda_d34c13f9b2fef67565400b6feddab80d> &>
        @ 0076FA15 absl::lts_20240116::base_internal::invoke<<lambda_d34c13f9b2fef67565400b6feddab80d> &>
        @ 00B9A0C5 absl::lts_20240116::internal_any_invocable::InvokeR<void,<lambda_d34c13f9b2fef67565400b6feddab80d> &,void>
        @ 00B9A43C absl::lts_20240116::internal_any_invocable::RemoteInvoker<0,void,<lambda_d34c13f9b2fef67565400b6feddab80d> &>
        @ 007E2889 absl::lts_20240116::internal_any_invocable::Impl<void __cdecl(void)>::operator()
        @ 00BA083A grpc_event_engine::experimental::SelfDeletingClosure::Run
        @ 00BFA5F2 grpc_event_engine::experimental::WorkStealingThreadPool::ThreadState::Step
        @ 00BFA049 grpc_event_engine::experimental::WorkStealingThreadPool::ThreadState::ThreadBody
        @ 00BFA9E8 <lambda_7a66d94a58a3d80cae6219849ef1bd25>::operator()
        @ 00BFAA2E <lambda_7a66d94a58a3d80cae6219849ef1bd25>::<lambda_invoker_cdecl>
        @ 0108468A `anonymous namespace'::ThreadInternalsWindows::thread_body
        @ 76957BA9 BaseThreadInitThunk
        @ 77C9BD2B RtlInitializeExceptionChain
        @ 77C9BCAF RtlClearBits

[mutex.cc : 1431] RAW: Acquiring absl::Mutex 01EC9B40(handshake) while holding  03F9EE20(conn state); a cycle in the historical lock ordering graph has been observed
[mutex.cc : 1432] RAW: Cycle:
[mutex.cc : 1446] RAW: mutex@01EC9B40 stack:
        @ 010E39C9 absl::lts_20240116::DebugOnlyDeadlockCheck
        @ 010DDFA4 absl::lts_20240116::Mutex::Lock
        @ 007872A0 absl::lts_20240116::MutexLock::MutexLock
        @ 00A048B3 grpc_core::HandshakeManager::Add
        @ 00D97A16 grpc_core::`anonymous namespace'::TCPConnectHandshakerFactory::AddHandshakers
        @ 00B2FF71 grpc_core::HandshakerRegistry::AddHandshakers
        @ 00C5D285 grpc_core::Chttp2Connector::Connect
        @ 00C3E0EE grpc_core::Subchannel::StartConnectingLocked
        @ 00C3D015 grpc_core::Subchannel::RequestConnection
        @ 00D7E2F0 grpc_core::ClientChannelFilter::SubchannelWrapper::RequestConnection
        @ 00E64FED grpc_core::`anonymous namespace'::PickFirst::SubchannelList::SubchannelData::RequestConnectionWithTimer
        @ 00E67557 grpc_core::`anonymous namespace'::PickFirst::SubchannelList::StartConnectingNextSubchannel
        @ 00E65E56 grpc_core::`anonymous namespace'::PickFirst::SubchannelList::SubchannelData::OnConnectivityStateChange
        @ 00E65451 grpc_core::`anonymous namespace'::PickFirst::SubchannelList::SubchannelData::Watcher::OnConnectivityStateChange
        @ 00D7B3FD grpc_core::ClientChannelFilter::SubchannelWrapper::WatcherWrapper::ApplyUpdateInControlPlaneWorkSerializer
        @ 00D784E6 <lambda_52bd2a765119f95b7bea0a42df380ec2>::operator()
        @ 00D6BB2B std::invoke<<lambda_52bd2a765119f95b7bea0a42df380ec2> &>
        @ 00D80F0A std::_Func_impl_no_alloc<<lambda_52bd2a765119f95b7bea0a42df380ec2>,void>::_Do_call
        @ 00AE3DC6 std::_Func_class<void>::operator()
        @ 00B564B1 grpc_core::WorkSerializer::LegacyWorkSerializer::DrainQueueOwned
        @ 00B55D64 grpc_core::WorkSerializer::LegacyWorkSerializer::Run
        @ 00B55A9D grpc_core::WorkSerializer::Run
        @ 00D38412 grpc_core::ClientChannelFilter::CheckConnectivityState
        @ 00CAA1E7 grpc_channel_check_connectivity_state
        @ 008FECDF grpc_core::CoreEnd2endTest::CheckConnectivityState
        @ 008F8495 grpc_core::`anonymous namespace'::CoreEnd2endTest_RetryHttp2Test_ConnectivityWatch::RunTest
        @ 008F7FFE grpc_core::`anonymous namespace'::CoreEnd2endTest_RetryHttp2Test_ConnectivityWatch::TestBody
        @ 0095CCD9 testing::internal::HandleSehExceptionsInMethodIfSupported<testing::Test,void>
        @ 0095C45D testing::internal::HandleExceptionsInMethodIfSupported<testing::Test,void>
        @ 00938F2A testing::Test::Run
        @ 009399D7 testing::TestInfo::Run
        @ 0093A290 testing::TestSuite::Run
        @ 0094084F testing::internal::UnitTestImpl::RunAllTests
        @ 0095CFA0 testing::internal::HandleSehExceptionsInMethodIfSupported<testing::internal::UnitTestImpl,bool>
        @ 0095CACD testing::internal::HandleExceptionsInMethodIfSupported<testing::internal::UnitTestImpl,bool>
        @ 0093AAFC testing::UnitTest::Run
        @ 0089BC2F RUN_ALL_TESTS
        @ 008997E2 main
        @ 013107B3 invoke_main
        @ 01310637 __scrt_common_main_seh

[mutex.cc : 1446] RAW: mutex@03F9EE20 stack:
        @ 010E39C9 absl::lts_20240116::DebugOnlyDeadlockCheck
        @ 010DDFA4 absl::lts_20240116::Mutex::Lock
        @ 007872A0 absl::lts_20240116::MutexLock::MutexLock
        @ 00B92528 grpc_event_engine::experimental::WindowsEventEngine::Connect
        @ 00BF45DC grpc_event_engine::experimental::event_engine_tcp_client_connect
        @ 00BADB12 tcp_connect
        @ 00AD53FC grpc_tcp_client_connect
        @ 00D97336 grpc_core::`anonymous namespace'::TCPConnectHandshaker::DoHandshake
        @ 00A052E5 grpc_core::HandshakeManager::CallNextHandshakerLocked
        @ 00A04DF3 grpc_core::HandshakeManager::DoHandshake
        @ 00C5D2F5 grpc_core::Chttp2Connector::Connect
        @ 00C3E0EE grpc_core::Subchannel::StartConnectingLocked
        @ 00C3D015 grpc_core::Subchannel::RequestConnection
        @ 00D7E2F0 grpc_core::ClientChannelFilter::SubchannelWrapper::RequestConnection
        @ 00E64FED grpc_core::`anonymous namespace'::PickFirst::SubchannelList::SubchannelData::RequestConnectionWithTimer
        @ 00E67557 grpc_core::`anonymous namespace'::PickFirst::SubchannelList::StartConnectingNextSubchannel
        @ 00E65E56 grpc_core::`anonymous namespace'::PickFirst::SubchannelList::SubchannelData::OnConnectivityStateChange
        @ 00E65451 grpc_core::`anonymous namespace'::PickFirst::SubchannelList::SubchannelData::Watcher::OnConnectivityStateChange
        @ 00D7B3FD grpc_core::ClientChannelFilter::SubchannelWrapper::WatcherWrapper::ApplyUpdateInControlPlaneWorkSerializer
        @ 00D784E6 <lambda_52bd2a765119f95b7bea0a42df380ec2>::operator()
        @ 00D6BB2B std::invoke<<lambda_52bd2a765119f95b7bea0a42df380ec2> &>
        @ 00D80F0A std::_Func_impl_no_alloc<<lambda_52bd2a765119f95b7bea0a42df380ec2>,void>::_Do_call
        @ 00AE3DC6 std::_Func_class<void>::operator()
        @ 00B564B1 grpc_core::WorkSerializer::LegacyWorkSerializer::DrainQueueOwned
        @ 00B55D64 grpc_core::WorkSerializer::LegacyWorkSerializer::Run
        @ 00B55A9D grpc_core::WorkSerializer::Run
        @ 00D38412 grpc_core::ClientChannelFilter::CheckConnectivityState
        @ 00CAA1E7 grpc_channel_check_connectivity_state
        @ 008FECDF grpc_core::CoreEnd2endTest::CheckConnectivityState
        @ 008F8495 grpc_core::`anonymous namespace'::CoreEnd2endTest_RetryHttp2Test_ConnectivityWatch::RunTest
        @ 008F7FFE grpc_core::`anonymous namespace'::CoreEnd2endTest_RetryHttp2Test_ConnectivityWatch::TestBody
        @ 0095CCD9 testing::internal::HandleSehExceptionsInMethodIfSupported<testing::Test,void>
        @ 0095C45D testing::internal::HandleExceptionsInMethodIfSupported<testing::Test,void>
        @ 00938F2A testing::Test::Run
        @ 009399D7 testing::TestInfo::Run
        @ 0093A290 testing::TestSuite::Run
        @ 0094084F testing::internal::UnitTestImpl::RunAllTests
        @ 0095CFA0 testing::internal::HandleSehExceptionsInMethodIfSupported<testing::internal::UnitTestImpl,bool>
        @ 0095CACD testing::internal::HandleExceptionsInMethodIfSupported<testing::internal::UnitTestImpl,bool>
        @ 0093AAFC testing::UnitTest::Run


[mutex.cc : 1454] RAW: dying due to potential deadlock
*** SIGABRT received at time=1707690409 ***
    @   00FF77AF  (unknown)  absl::lts_20240116::WriteFailureInfo
    @   00FF7904  (unknown)  absl::lts_20240116::AbslFailureSignalHandler
    @   6D3428B4  (unknown)  raise
    @   6D343ED2  (unknown)  abort
    @   0114348A  (unknown)  absl::lts_20240116::raw_log_internal::`anonymous namespace'::RawLogVA
    @   01143093  (unknown)  absl::lts_20240116::raw_log_internal::RawLog
    @   010E3823  (unknown)  absl::lts_20240116::DeadlockCheck
    @   010E39C9  (unknown)  absl::lts_20240116::DebugOnlyDeadlockCheck
    @   010DDFA4  (unknown)  absl::lts_20240116::Mutex::Lock
    @   007872A0  (unknown)  absl::lts_20240116::MutexLock::MutexLock
    @   00A053EB  (unknown)  grpc_core::HandshakeManager::CallNextHandshakerFn
    @   009D6B14  (unknown)  exec_ctx_run
    @   009D6600  (unknown)  grpc_core::ExecCtx::Flush
    @   0078B9DD  (unknown)  grpc_core::ExecCtx::~ExecCtx
    @   00BF4B01  (unknown)  <lambda_41920641b739d4c7bb98a916290a31a8>::operator()
    @   00BF505B  (unknown)  absl::lts_20240116::base_internal::Callable::Invoke<<lambda_41920641b739d4c7bb98a916290a31a8> &,absl::lts_20240116::StatusOr<std::unique_ptr<grpc_event_engine::experimental::EventEngine::Endpoint,std::default_delete<grpc_event_engine::experimental::EventEngine::Endpoint> > > >
    @   00BF5162  (unknown)  absl::lts_20240116::base_internal::invoke<<lambda_41920641b739d4c7bb98a916290a31a8> &,absl::lts_20240116::StatusOr<std::unique_ptr<grpc_event_engine::experimental::EventEngine::Endpoint,std::default_delete<grpc_event_engine::experimental::EventEngine::Endpoint> > > >
    @   00BF50A2  (unknown)  absl::lts_20240116::internal_any_invocable::InvokeR<void,<lambda_41920641b739d4c7bb98a916290a31a8> &,absl::lts_20240116::StatusOr<std::unique_ptr<grpc_event_engine::experimental::EventEngine::Endpoint,std::default_delete<grpc_event_engine::experimental::EventEngine::Endpoint> > >,void>
    @   00BF50E7  (unknown)  absl::lts_20240116::internal_any_invocable::LocalInvoker<0,void,<lambda_41920641b739d4c7bb98a916290a31a8> &,absl::lts_20240116::StatusOr<std::unique_ptr<grpc_event_engine::experimental::EventEngine::Endpoint,std::default_delete<grpc_event_engine::experimental::EventEngine::Endpoint> > > >
    @   007B0B33  (unknown)  absl::lts_20240116::internal_any_invocable::Impl<void __cdecl(absl::lts_20240116::StatusOr<std::unique_ptr<grpc_event_engine::experimental::EventEngine::Endpoint,std::default_delete<grpc_event_engine::experimental::EventEngine::Endpoint> > >)>::operator()
    @   00B94BDA  (unknown)  <lambda_d34c13f9b2fef67565400b6feddab80d>::operator()
    @   00B99DA6  (unknown)  absl::lts_20240116::base_internal::Callable::Invoke<<lambda_d34c13f9b2fef67565400b6feddab80d> &>
    @   0076FA15  (unknown)  absl::lts_20240116::base_internal::invoke<<lambda_d34c13f9b2fef67565400b6feddab80d> &>
    @   00B9A0C5  (unknown)  absl::lts_20240116::internal_any_invocable::InvokeR<void,<lambda_d34c13f9b2fef67565400b6feddab80d> &,void>
    @   00B9A43C  (unknown)  absl::lts_20240116::internal_any_invocable::RemoteInvoker<0,void,<lambda_d34c13f9b2fef67565400b6feddab80d> &>
    @   007E2889  (unknown)  absl::lts_20240116::internal_any_invocable::Impl<void __cdecl(void)>::operator()
    @   00BA083A  (unknown)  grpc_event_engine::experimental::SelfDeletingClosure::Run
    @   00BFA5F2  (unknown)  grpc_event_engine::experimental::WorkStealingThreadPool::ThreadState::Step
    @   00BFA049  (unknown)  grpc_event_engine::experimental::WorkStealingThreadPool::ThreadState::ThreadBody
    @   00BFA9E8  (unknown)  <lambda_7a66d94a58a3d80cae6219849ef1bd25>::operator()
    @   00BFAA2E  (unknown)  <lambda_7a66d94a58a3d80cae6219849ef1bd25>::<lambda_invoker_cdecl>
    @   0108468A  (unknown)  `anonymous namespace'::ThreadInternalsWindows::thread_body
PS D:\code\cpp\grpc>
```