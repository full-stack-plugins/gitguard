//! Compile as an example against the pinned TestGuard crate, never a replacement parser.
use guardengine::integration::{RunBinding,Producer};
use serde::Serialize;
use std::{path::Path,fs};
use testguard::{adapters::{ExecutorProfile,RawArtifactSet,cargo},plan::{FrozenPlan,Binding},obligation::ObligationSet,report::{AttemptRecord,normalize::{ArtifactRef,canonical_digest,bytes_digest},engine_adapter::{self,CAPABILITY,ANALYZER_ID,MAPPING_VERSION},transport::{self,FixtureInvocation},envelope::verify_bundle}};
fn write(p:impl AsRef<Path>,v:&impl Serialize){fs::write(p,serde_json::to_vec(v).unwrap()).unwrap();}
fn read<T:serde::de::DeserializeOwned>(p:impl AsRef<Path>)->T{serde_json::from_slice(&fs::read(p).unwrap()).unwrap()}
fn main(){
 let a:Vec<_>=std::env::args().collect();assert_eq!(a.len(),4,"prepare|finish REPO OUTPUT");let mode=&a[1];let root=Path::new(&a[2]);let out=Path::new(&a[3]);let dst=out.join("testguard");fs::create_dir_all(&dst).unwrap();
 if mode=="prepare"{
  let mut b:RunBinding=read(out.join("binding.json"));let baseline=bytes_digest(&fs::read(root.join("baseline.json")).unwrap());b.baseline_digest=Some(format!("sha256:{baseline}"));
  let profile=ExecutorProfile{tool:"cargo".into(),version:"1.99.0".into(),protocol:"libtest-pretty-v1".into(),target:"cross-guard-app:lib".into(),features:vec![],parameters:"tests::required_answer".into(),environment:"linux-fixture-cargo-1.99".into()};
  let test_id=canonical_digest(&("tests::required_answer",&profile.target,&profile.features,&profile.parameters,&profile.environment)).unwrap();
  let obligations=ObligationSet::parse(&serde_json::json!({"schema_version":"testguard.local/v1","capability":"local-fixture","approval_ref":"fixture:baseline-controller","revision":"1","baseline_digest":baseline,"requirements":b.requirement_ids,"sources":[{"id":"AC-R","requirement_id":"demo:R","kind":"acceptance"}],"environments":[profile.environment],"obligations":[{"id":"O-R","source_ids":["AC-R"],"test_id":test_id,"environments":[profile.environment]}]}).to_string()).unwrap();
  let plan=FrozenPlan::freeze(&obligations,Binding{repository:b.repo_id.clone(),candidate:b.candidate_oid.clone(),base:b.base_oid.clone(),source_digest:b.source_snapshot_digest.strip_prefix("sha256:").unwrap().into(),policy_digest:canonical_digest(&profile).unwrap()}).unwrap();
  let invocation=FixtureInvocation{capability:CAPABILITY.into(),run_id:"testguard-native-cross".into(),binding:b.clone(),started_at:"2026-10-09T00:00:00Z".into(),finished_at:"2026-10-09T00:00:01Z".into()};transport::prepare(&plan,invocation.clone()).unwrap();
  let contract=serde_json::to_vec(&engine_adapter::contract(&plan).unwrap()).unwrap();fs::write(dst.join("prepared-contract.json"),&contract).unwrap();
  write(dst.join("expected.json"),&serde_json::json!({"binding":b,"producer":Producer{guard:"TestGuard".into(),version:env!("CARGO_PKG_VERSION").into(),analyzer_id:ANALYZER_ID.into(),analyzer_version:MAPPING_VERSION.into()},"required_scopes":engine_adapter::required_scopes(&plan).unwrap(),"contract_digest":format!("sha256:{}",bytes_digest(&contract))}));
  write(dst.join("plan.json"),&plan);write(dst.join("invocation.json"),&invocation);write(dst.join("profile.json"),&profile);write(dst.join("obligations.json"),&obligations);
 }else if mode=="finish"{
  let plan:FrozenPlan=read(dst.join("plan.json"));let invocation:FixtureInvocation=read(dst.join("invocation.json"));let profile:ExecutorProfile=read(dst.join("profile.json"));
  let output=fs::read_to_string(dst.join("stdout.raw")).unwrap();let metadata:serde_json::Value=read(dst.join("capture.json"));let exit=metadata["exit_code"].as_i64().unwrap() as i32;
  let artifact=ArtifactRef::from_bytes(&invocation.run_id,"actual-native-cargo",output.as_bytes()).unwrap();
  let raw=RawArtifactSet{attempt_id:invocation.run_id.clone(),inventory:fs::read_to_string(dst.join("inventory.raw")).unwrap(),output,artifact:artifact.clone(),exit_code:Some(exit),interrupted:false};
  let attempt=AttemptRecord{schema_version:"testguard.local/v1".into(),attempt_id:invocation.run_id.clone(),plan_digest:canonical_digest(&plan).unwrap(),exit_code:Some(exit),finished:true,observations:cargo::parse(&raw,&profile).unwrap(),artifacts:vec![artifact]};
  write(dst.join("attempt.json"),&attempt);let bundle=transport::prepare(&plan,invocation).unwrap().complete(&attempt,&[],&[]).unwrap();verify_bundle(&plan,&bundle).unwrap();
  write(dst.join("bundle.json"),&bundle);write(dst.join("envelope.json"),&bundle.envelope);
  for (name,reference) in [("contract",bundle.envelope.artifacts.contract.as_ref().unwrap()),("facts",bundle.envelope.artifacts.facts.as_ref().unwrap()),("report",bundle.envelope.artifacts.report.as_ref().unwrap()),("domain",&bundle.envelope.artifacts.domain[0])]{fs::write(dst.join(format!("{name}.json")),&bundle.artifacts[&reference.uri]).unwrap();}
  assert_eq!(fs::read(dst.join("prepared-contract.json")).unwrap(),fs::read(dst.join("contract.json")).unwrap());
 }else{panic!("unknown mode")}
}
