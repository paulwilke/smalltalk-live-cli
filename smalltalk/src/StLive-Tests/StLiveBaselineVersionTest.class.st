Class {
	#name : 'StLiveBaselineVersionTest',
	#superclass : 'StLiveTestCase',
	#instVars : [
		'version'
	],
	#package : 'StLive-Tests'
}

{ #category : 'running' }
StLiveBaselineVersionTest >> setUp [
	"A small BaselineOf in a test package: Core; Tests (requires Core); Solo (requires Core, group of one package)."
	super setUp.
	self
		compile: 'baseline: spec
	<baseline>
	spec for: #common do: [
		spec package: ''StLiveTestsCore''.
		spec package: ''StLiveTestsTests'' with: [ spec requires: #(''StLiveTestsCore'') ].
		spec package: ''StLiveTestsSolo'' with: [ spec requires: #(''StLiveTestsCore'') ] ].
	spec group: ''default'' with: #(''StLiveTestsCore'').
	spec group: ''Tests'' with: #(''default'' ''StLiveTestsTests'').
	spec group: ''Solo'' with: #(''StLiveTestsSolo'')'
		in: (self fixtureClassNamed: 'BaselineOfStLiveTestsFixture' superclass: BaselineOf package: 'StLiveTestsFixture').
	version := (Smalltalk at: #BaselineOfStLiveTestsFixture) version
]

{ #category : 'helpers' }
StLiveBaselineVersionTest >> namesOf: aCollection [
	^ (aCollection collect: [ :each | each asString ]) asSet
]

{ #category : 'versions' }
StLiveBaselineVersionTest >> testFixtureBaselineIsFoundByName [
	self assert: (commands baselineVersionOf: 'StLiveTestsFixture') notNil
]

{ #category : 'versions' }
StLiveBaselineVersionTest >> testMissingBaselineHasNoVersion [
	self assert: (commands baselineVersionOf: 'StLiveTestsNoSuchBaseline') isNil
]

{ #category : 'versions' }
StLiveBaselineVersionTest >> testPackageNamesListEveryPackageOfTheBaseline [
	self assert: (self namesOf: (commands packageNamesOfVersion: version)) equals: #('StLiveTestsCore' 'StLiveTestsTests' 'StLiveTestsSolo') asSet
]

{ #category : 'versions' }
StLiveBaselineVersionTest >> testPackageNamesOfNoVersionAreEmpty [
	self assert: (commands packageNamesOfVersion: nil) equals: #()
]

{ #category : 'versions' }
StLiveBaselineVersionTest >> testGroupMembershipsExpandGroupsOfGroups [
	| memberships |
	memberships := commands groupMembershipsOf: version.
	self assert: (self namesOf: (memberships at: 'default')) equals: #('StLiveTestsCore') asSet.
	self assert: (self namesOf: (memberships at: 'Tests')) equals: #('StLiveTestsCore' 'StLiveTestsTests') asSet
]

{ #category : 'versions' }
StLiveBaselineVersionTest >> testGroupMembershipsOfNoVersionAreEmpty [
	self assert: (commands groupMembershipsOf: nil) isEmpty
]

{ #category : 'loaded for groups' }
StLiveBaselineVersionTest >> testDefaultGroupLoadsTheCoreOnly [
	self assert: (self namesOf: (commands packagesLoadedForGroups: #() version: version)) equals: #('StLiveTestsCore') asSet
]

{ #category : 'loaded for groups' }
StLiveBaselineVersionTest >> testTestsGroupLoadsItsPackagesAndTheDefaultGroup [
	self assert: (self namesOf: (commands packagesLoadedForGroups: #('Tests') version: version)) equals: #('StLiveTestsCore' 'StLiveTestsTests') asSet
]

{ #category : 'loaded for groups' }
StLiveBaselineVersionTest >> testRequiredPackagesAreLoadedWithTheirGroup [
	self assert: (self namesOf: (commands packagesLoadedForGroups: #('Solo') version: version)) equals: #('StLiveTestsSolo' 'StLiveTestsCore') asSet
]

{ #category : 'loaded for groups' }
StLiveBaselineVersionTest >> testUnknownGroupLoadsNothing [
	self assert: (commands packagesLoadedForGroups: #('NoSuchGroup') version: version) isEmpty
]

{ #category : 'loaded for groups' }
StLiveBaselineVersionTest >> testNoVersionLoadsNothing [
	self assert: (commands packagesLoadedForGroups: #() version: nil) isEmpty
]

{ #category : 'loaded for groups' }
StLiveBaselineVersionTest >> testKnownPackagesOfABaselineWithoutRecordComeFromItsVersion [
	self assert: (self namesOf: (commands knownBaselinePackagesOf: 'StLiveTestsFixture')) equals: #('StLiveTestsCore' 'StLiveTestsTests' 'StLiveTestsSolo') asSet
]

{ #category : 'loaded for groups' }
StLiveBaselineVersionTest >> testNotReloadedUsesTheGroupsOfTheBaseline [
	| report |
	self loadedPackage: 'StLiveTestsTests'.
	report := commands
		notReloadedPackagesOf: (commands packageNamesOfVersion: version)
		requested: (commands packagesLoadedForGroups: #() version: version)
		memberships: (commands groupMembershipsOf: version).
	self assert: report size equals: 1.
	self assert: ((report first) at: 'package') equals: 'StLiveTestsTests'.
	self assert: ((report first) at: 'hint') equals: 'load again with --group Tests'
]
