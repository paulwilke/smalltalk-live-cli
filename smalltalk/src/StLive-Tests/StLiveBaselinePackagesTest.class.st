Class {
	#name : 'StLiveBaselinePackagesTest',
	#superclass : 'StLiveTestCase',
	#package : 'StLive-Tests'
}

{ #category : 'stale packages' }
StLiveBaselinePackagesTest >> testStaleIsKnownLoadedPackageNoLongerListed [
	self loadedPackage: 'StLiveTestsA'.
	self loadedPackage: 'StLiveTestsB'.
	self assert: (commands stalePackagesOf: #('StLiveTestsA' 'StLiveTestsB') current: #('StLiveTestsA')) equals: #('StLiveTestsB')
]

{ #category : 'stale packages' }
StLiveBaselinePackagesTest >> testStaleSkipsKnownPackagesThatAreNotLoaded [
	self assert: (commands stalePackagesOf: #('StLiveTestsNeverLoaded') current: #()) equals: #()
]

{ #category : 'stale packages' }
StLiveBaselinePackagesTest >> testStaleIsSortedWithoutDuplicates [
	self loadedPackage: 'StLiveTestsA'.
	self loadedPackage: 'StLiveTestsB'.
	self assert: (commands stalePackagesOf: #('StLiveTestsB' 'StLiveTestsA' 'StLiveTestsB') current: #()) equals: #('StLiveTestsA' 'StLiveTestsB')
]

{ #category : 'stale packages' }
StLiveBaselinePackagesTest >> testStaleOfNothingKnownIsEmpty [
	self assert: (commands stalePackagesOf: #() current: #('StLiveTestsA')) equals: #()
]

{ #category : 'not reloaded' }
StLiveBaselinePackagesTest >> testNotReloadedHintNamesTheGroupThatLoadsIt [
	| memberships report |
	self loadedPackage: 'StLiveTestsT'.
	memberships := OrderedDictionary new.
	memberships at: 'Tests' put: #('StLiveTestsT') asSet.
	report := commands notReloadedPackagesOf: #('StLiveTestsT') requested: #('StLiveTestsCore') memberships: memberships.
	self assert: report size equals: 1.
	self assert: ((report first) at: 'package') equals: 'StLiveTestsT'.
	self assert: ((report first) at: 'hint') equals: 'load again with --group Tests'
]

{ #category : 'not reloaded' }
StLiveBaselinePackagesTest >> testNotReloadedHintListsEveryGroupInOrder [
	| memberships report |
	self loadedPackage: 'StLiveTestsT'.
	memberships := OrderedDictionary new.
	memberships at: 'Tests' put: #('StLiveTestsT') asSet.
	memberships at: 'Dev' put: #('StLiveTestsT') asSet.
	report := commands notReloadedPackagesOf: #('StLiveTestsT') requested: #() memberships: memberships.
	self assert: ((report first) at: 'hint') equals: 'load again with --group Dev --group Tests'
]

{ #category : 'not reloaded' }
StLiveBaselinePackagesTest >> testNotReloadedHintWhenNoGroupListsThePackage [
	| memberships report |
	self loadedPackage: 'StLiveTestsT'.
	memberships := OrderedDictionary new.
	memberships at: 'Tests' put: Set new.
	report := commands notReloadedPackagesOf: #('StLiveTestsT') requested: #() memberships: memberships.
	self assert: ((report first) at: 'hint') equals: 'no group of this baseline lists it'
]

{ #category : 'not reloaded' }
StLiveBaselinePackagesTest >> testNotReloadedWithoutMembershipsSaysNoGroupListsIt [
	| report |
	self loadedPackage: 'StLiveTestsT'.
	report := commands notReloadedPackagesOf: #('StLiveTestsT') requested: #() memberships: OrderedDictionary new.
	self assert: ((report first) at: 'hint') equals: 'no group of this baseline lists it'
]

{ #category : 'not reloaded' }
StLiveBaselinePackagesTest >> testNotReloadedSkipsPackagesThatWereRequested [
	| memberships |
	self loadedPackage: 'StLiveTestsT'.
	memberships := OrderedDictionary new.
	memberships at: 'Tests' put: #('StLiveTestsT') asSet.
	self assert: (commands notReloadedPackagesOf: #('StLiveTestsT') requested: #('StLiveTestsT') memberships: memberships) isEmpty
]

{ #category : 'not reloaded' }
StLiveBaselinePackagesTest >> testNotReloadedSkipsPackagesThatAreNotLoaded [
	self assert: (commands notReloadedPackagesOf: #('StLiveTestsNeverLoaded') requested: #() memberships: OrderedDictionary new) isEmpty
]

{ #category : 'remember' }
StLiveBaselinePackagesTest >> testRememberKeepsKnownPackagesThatAreStillLoaded [
	self loadedPackage: 'StLiveTestsA'.
	commands rememberBaselinePackages: 'StLiveTestsFixture' known: #('StLiveTestsA' 'StLiveTestsB') current: #('StLiveTestsC') unloaded: #().
	self assert: (commands knownBaselinePackagesOf: 'StLiveTestsFixture') equals: #('StLiveTestsA' 'StLiveTestsC')
]

{ #category : 'remember' }
StLiveBaselinePackagesTest >> testRememberForgetsUnloadedPackages [
	self loadedPackage: 'StLiveTestsA'.
	commands rememberBaselinePackages: 'StLiveTestsFixture' known: #('StLiveTestsA') current: #() unloaded: #('StLiveTestsA').
	self assert: (commands knownBaselinePackagesOf: 'StLiveTestsFixture') equals: #()
]

{ #category : 'remember' }
StLiveBaselinePackagesTest >> testRememberedPackagesStayStaleUntilPruned [
	| known |
	self loadedPackage: 'StLiveTestsA'.
	commands rememberBaselinePackages: 'StLiveTestsFixture' known: #('StLiveTestsA') current: #() unloaded: #().
	known := commands knownBaselinePackagesOf: 'StLiveTestsFixture'.
	self assert: (commands stalePackagesOf: known current: #()) equals: #('StLiveTestsA')
]

{ #category : 'remember' }
StLiveBaselinePackagesTest >> testRecordsAreKeptPerBaseline [
	commands rememberBaselinePackages: 'StLiveTestsFixture' known: #() current: #('StLiveTestsC') unloaded: #().
	self assert: (commands knownBaselinePackagesOf: 'StLiveTestsOther') equals: #()
]
