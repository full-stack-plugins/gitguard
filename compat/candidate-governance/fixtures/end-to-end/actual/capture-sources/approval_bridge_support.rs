//! Trusted fixture generator only. No production approval implementation.
use std::{collections::{BTreeMap,BTreeSet},path::Path};
use specguard::{baseline::*,model::*,source::*,integration::{approval::*,producer::*,baseline_review::{BaselineReview,ReviewRule},freshness::*,runtime::CancellationToken}};
use guardengine::{*,integration::{Producer,eligibility::*}};
const TIME:&str="2026-10-09T00:00:00Z";
pub const NOW:i64=2_000_000_000;
fn read<T:serde::de::DeserializeOwned>(p:impl AsRef<Path>)->T{serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()}
fn write(p:impl AsRef<Path>,v:&impl serde::Serialize){std::fs::write(p,serde_json::to_vec(v).unwrap()).unwrap()}
fn key()->Identity{Identity{namespace:"demo".into(),id:"R".into()}}
fn source_policy()->SourcePolicy{SourcePolicy{api_version:Version::V1,roots:vec![SourceRoot{path:"specs".into(),namespace:"demo".into(),format:"markdown-explicit/v1".into(),authority:"primary".into()}],limits:Limits::default()}}
fn oid(root:&Path)->String{String::from_utf8(std::process::Command::new("/usr/bin/git").arg("-C").arg(root).args(["rev-parse","HEAD"]).output().unwrap().stdout).unwrap().trim().into()}
fn snapshot(root:&Path)->SourceSnapshot{
 let oid=oid(root);let inventory=discover(root,&source_policy()).unwrap();
 let s=freeze(root,&inventory,CandidateBinding{candidate_oid:oid.clone(),base_oid:serde_json::from_slice::<serde_json::Value>(&std::fs::read(root.join("baseline.json")).unwrap()).unwrap().get("sourceRevision").and_then(|v|v.as_str()).unwrap_or(&oid).into(),object_format:"sha1".into()}).unwrap();
 let repo=gitguard::Repository::discover(root,"cross-guard-repo").unwrap();
 let files=repo.read_commit_files(&oid).unwrap();
 for (name,contents) in &s.contents {let file=files.iter().find(|f|f.path()==name.as_bytes()).expect("snapshot source in immutable candidate");assert_eq!(file.contents(),contents);}
 s
}
fn mapping()->ProtectedMapping{
 let kinds=[specguard::rules::FindingKind::Duplicate,specguard::rules::FindingKind::MissingRequirement,specguard::rules::FindingKind::MissingAcceptance,specguard::rules::FindingKind::BrokenReference];
 let mut entries=vec![];let mut rules=vec![];
 for (i,kind) in kinds.into_iter().enumerate(){let rule_id=format!("structure-{i}");let object=format!("finding-{i}");entries.push(MappingEntry{key:key(),kind,rule_id:rule_id.clone(),subject:"demo:R".into(),predicate:"spec_violation".into(),object:object.clone()});rules.push(GuardRule{id:rule_id,description:"protected structural rule".into(),enforcement:Enforcement::Enforce,assertion:GuardAssertion::ForbidRelation{subject:"demo:R".into(),predicate:"spec_violation".into(),object}});}
 ProtectedMapping{entries,contract:GuardContract{api_version:API_VERSION.into(),kind:"GuardContract".into(),metadata:ContractMetadata{id:"approval-bridge-structure".into(),revision:"1".into()},spec:ContractSpec{rules}}}
}
fn review(root:&Path)->BaselineReview{
 let baseline:ApprovedBaseline=read(root.join("baseline.json"));
 let mut rules=vec![];let mut engine=vec![];
 for (kind,name) in [(ChangeKind::TextReview,"text_review"),(ChangeKind::AcceptanceReview,"acceptance_review")] {let rule_id=format!("review-{name}");rules.push(ReviewRule{key:key(),kind,rule_id:rule_id.clone(),subject:"demo:R".into(),predicate:"baseline_change".into(),object:name.into()});engine.push(GuardRule{id:rule_id,description:"protected baseline review".into(),enforcement:Enforcement::Review,assertion:GuardAssertion::ForbidRelation{subject:"demo:R".into(),predicate:"baseline_change".into(),object:name.into()}});}
 BaselineReview{api_version:specguard::integration::baseline_review::Version::V1,baseline,rules,contract:GuardContract{api_version:API_VERSION.into(),kind:"GuardContract".into(),metadata:ContractMetadata{id:"approval-bridge-review".into(),revision:"1".into()},spec:ContractSpec{rules:engine}}}
}
pub struct BaselineAuthority(pub Authentication);
impl ApprovalValidationPort for BaselineAuthority{fn profile(&self)->Profile{Profile::Fixture}fn validate(&self,_:&ApprovedBaseline)->Result<Authentication,ApprovalError>{Ok(self.0.clone())}}
pub fn baseline_authority(b:&ApprovedBaseline)->BaselineAuthority{BaselineAuthority(Authentication{issuer:"fixture-baseline-issuer".into(),purpose:"specification-baseline".into(),repository:b.repository.clone(),scope:b.scope.clone(),baseline_digest:digest(b),policy_digest:b.policy_digest.clone(),issued_at:NOW-10,expires_at:NOW+100,revoked:false})}
pub fn prepared(root:&Path)->(PreparedRun,BaselineReview,EligibilityPolicy){
 let s=snapshot(root);let review=review(root);authenticate(&review.baseline,&baseline_authority(&review.baseline),Profile::Fixture,NOW).unwrap();
 let inv=Invocation{run_id:"sg-actual-review".into(),started_at:TIME.into(),repo_id:"cross-guard-repo".into(),task_id:"cross-guard-task".into(),worktree_id:"fixture-worktree".into(),candidate_oid:Some(s.binding.candidate_oid.clone()),base_oid:Some(s.binding.base_oid.clone()),merge_group_id:None,baseline_digest:Some(digest(&review.baseline))};
 let prepared=prepare_baseline_review(root,&s,inv,&BTreeSet::from([key()]),&mapping(),&review).unwrap();
 let mut contract=mapping().contract;contract.spec.rules.extend(review.contract.spec.rules.clone());
 let policy=EligibilityPolicy{binding:prepared.binding().clone(),producer:Producer{guard:"SpecGuard".into(),version:"0.1.0".into(),analyzer_id:"specguard.structural".into(),analyzer_version:"1".into()},required_scopes:vec!["profile:specguard.baseline-review/v1".into(),"profile:specguard.structural/v1".into(),"requirement:demo:R".into(),"source:specs/a.md".into()],contract_digest:digest(&contract),action:"commit".into(),producer_principals:BTreeSet::from(["fixture-ci".into()]),approval_principals:BTreeMap::from([("review".into(),BTreeSet::from(["fixture-reviewer".into()]))])};
 (prepared,review,policy)
}
pub fn run(mode:&str,root:&Path,out:&Path){
 let dst=out.join("specguard");std::fs::create_dir_all(&dst).unwrap();
 if mode=="sg-baseline" {
  let s=snapshot(root);let graph=specguard::graph::build_graph(specguard::parser::parse(&s));assert!(graph.complete());
  let b=ApprovedBaseline{api_version:Version::V1,repository:"cross-guard-repo".into(),scope:BTreeSet::from([key()]),source_revision:oid(root),source_digest:s.digest,graph_digest:digest(&graph),policy_digest:digest(&mapping()),approval_ref:"fixture:baseline-before-candidate".into(),effective_from:NOW-100,expires_at:NOW+1000,state:BaselineState::Approved,graph};
  authenticate(&b,&baseline_authority(&b),Profile::Fixture,NOW).unwrap();write(root.join("baseline.json"),&b);write(dst.join("baseline.json"),&b);
 } else if mode=="sg-prepare" {
  let (prepared,review,p)=prepared(root);let mut c=mapping().contract;c.spec.rules.extend(review.contract.spec.rules.clone());
  write(dst.join("prepared-contract.json"),&c);write(dst.join("review.json"),&review);write(dst.join("mapping.json"),&mapping());write(dst.join("snapshot.json"),&snapshot(root));
  write(dst.join("expected.json"),&serde_json::json!({"binding":prepared.binding(),"producer":p.producer,"required_scopes":p.required_scopes,"contract_digest":p.contract_digest}));
 } else if mode=="sg-finish" {
  let (prepared,_,policy)=prepared(root);let expected=CurrentExpectation::freeze(&prepared,1,&policy,Profile::Fixture).unwrap();
  let target=prepared.work_key().target().clone();let mut history=RunHistory::default();let complete=history.register(prepared,0).unwrap().execute("2026-10-09T00:00:01Z",&CancellationToken::new(),|_|{}).unwrap();
  assert_eq!(complete.output().envelope.decision,Some(Decision::RequireApproval));complete.output().verify().unwrap();let original=serde_json::to_vec(complete.output()).unwrap();
  history.append(&complete).unwrap();history.publish(&complete).unwrap();
  let attached=history.attach_current(&expected,&["fixture:actual-spec-review".into()]).unwrap();
  assert_eq!(attached.envelope().decision,Some(Decision::RequireApproval));
  std::fs::write(dst.join("original.json"),&original).unwrap();write(dst.join("envelope.json"),attached.envelope());
  for (name,bytes) in [("contract",attached.contract_bytes()),("facts",attached.facts_bytes()),("report",attached.report_bytes()),("domain",attached.domain_bytes())]{std::fs::write(dst.join(format!("{name}.json")),bytes.unwrap()).unwrap();}
  assert_eq!(serde_json::to_vec(history.current(&target).unwrap().output()).unwrap(),original);
  assert_eq!(std::fs::read(dst.join("prepared-contract.json")).unwrap(),std::fs::read(dst.join("contract.json")).unwrap());
 }else{panic!("unknown SG mode")}
}
