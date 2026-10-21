Class {
	#name : 'StLiveUndeclaredReferencesTest',
	#superclass : 'StLiveTestCase',
	#package : 'StLive-Tests'
}

{ #category : 'running' }
StLiveUndeclaredReferencesTest >> tearDown [
	"The fixture methods reference a name that nothing defines: Pharo keeps it in Undeclared, so it is removed here
	(after the fixture classes, which the superclass removes)."
	super tearDown.
	Undeclared removeKey: #StLiveTestsNoSuchGlobal ifAbsent: [ nil ]
]

{ #category : 'helpers' }
StLiveUndeclaredReferencesTest >> compileQuietly: aSource in: aClass [
	"Compiling a reference to an unknown name signals a warning; the test wants the method, not the warning."
	^ [ self compile: aSource in: aClass ] on: Warning do: [ :w | w resume ]
]

{ #category : 'undeclared' }
StLiveUndeclaredReferencesTest >> testReferenceToAMissingGlobalIsReportedWithItsMethod [
	| cls found |
	cls := self fixtureClassNamed: 'StLiveTestsUndeclaredThing' superclass: Object package: 'StLiveTestsUndeclared'.
	self compileQuietly: 'probe ^ StLiveTestsNoSuchGlobal new' in: cls.
	self compileQuietly: 'known ^ Object new' in: cls.
	found := commands undeclaredReferencesIn: #('StLiveTestsUndeclared').
	self assert: found size equals: 1.
	self assert: ((found first) at: 'name') equals: 'StLiveTestsNoSuchGlobal'.
	self assert: ((found first) at: 'methods') equals: #('StLiveTestsUndeclaredThing>>probe').
	self assert: ((found first) at: 'method_count') equals: 1
]

{ #category : 'undeclared' }
StLiveUndeclaredReferencesTest >> testOnlyTheGivenPackagesAreScanned [
	| cls |
	cls := self fixtureClassNamed: 'StLiveTestsUndeclaredThing' superclass: Object package: 'StLiveTestsUndeclared'.
	self compileQuietly: 'probe ^ StLiveTestsNoSuchGlobal new' in: cls.
	self assert: (commands undeclaredReferencesIn: #('StLiveTestsSomeOtherPackage' 'StLiveTestsNoSuchPackage')) isEmpty
]

{ #category : 'undeclared' }
StLiveUndeclaredReferencesTest >> testCleanPackageHasNoUndeclaredNames [
	self loadedPackage: 'StLiveUndeclaredClean'.
	self assert: (commands undeclaredReferencesIn: #('StLiveUndeclaredClean')) isEmpty
]

{ #category : 'undeclared' }
StLiveUndeclaredReferencesTest >> testMethodsListedPerNameAreCapped [
	| cls found |
	cls := self fixtureClassNamed: 'StLiveTestsUndeclaredThing' superclass: Object package: 'StLiveTestsUndeclared'.
	1 to: 12 do: [ :i |
		self compileQuietly: 'probe' , i printString , ' ^ StLiveTestsNoSuchGlobal new' in: cls ].
	found := commands undeclaredReferencesIn: #('StLiveTestsUndeclared').
	self assert: ((found first) at: 'method_count') equals: 12.
	self assert: ((found first) at: 'methods') size equals: 10
]
