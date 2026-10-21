"
Base class of the StLive unit tests. Each test gets a fresh server that does not listen, and a command object on it.
Fixture classes, traits and packages a test creates are removed in tearDown. Fixture changes are not recorded as
changes of the image's own server (quietly:), so loading or running these tests leaves no stlive changes behind.
"
Class {
	#name : 'StLiveTestCase',
	#superclass : 'TestCase',
	#instVars : [
		'server',
		'commands',
		'fixtureClasses',
		'fixturePackages'
	],
	#package : 'StLive-Tests'
}

{ #category : 'running' }
StLiveTestCase >> setUp [
	super setUp.
	server := StLiveServer new.
	commands := StLiveCommands on: server.
	fixtureClasses := OrderedCollection new.
	fixturePackages := OrderedCollection new
]

{ #category : 'running' }
StLiveTestCase >> tearDown [
	fixtureClasses ifNotNil: [
		self quietly: [
			fixtureClasses reverseDo: [ :each | each removeFromSystem ].
			fixturePackages do: [ :name |
				(PackageOrganizer default packageNamed: name ifAbsent: [ nil ]) ifNotNil: [ :pk | pk removeFromSystem ] ] ] ].
	server ifNotNil: [ SystemAnnouncer uniqueInstance unsubscribe: server ].
	super tearDown
]

{ #category : 'helpers' }
StLiveTestCase >> quietly: aBlock [
	"Runs the block without recording its changes in the image's own server (when one runs in this image)."
	^ StLiveServer current suppressRecordingDuring: aBlock
]

{ #category : 'helpers' }
StLiveTestCase >> rememberPackage: aName [
	(fixturePackages includes: aName) ifFalse: [ fixturePackages add: aName ]
]

{ #category : 'helpers' }
StLiveTestCase >> fixtureClassNamed: aString superclass: aClass package: aName [
	| cls |
	self rememberPackage: aName.
	cls := self quietly: [
		self class classInstaller make: [ :builder |
			builder superclass: aClass; name: aString asSymbol; package: aName ] ].
	fixtureClasses add: cls.
	^ cls
]

{ #category : 'helpers' }
StLiveTestCase >> fixtureTraitNamed: aString package: aName [
	| trait |
	self rememberPackage: aName.
	trait := self quietly: [ (Trait << aString asSymbol) package: aName; install ].
	fixtureClasses add: trait.
	^ trait
]

{ #category : 'helpers' }
StLiveTestCase >> compile: aSource in: aClass [
	"The CompiledMethod (compile:classified: itself answers the selector)."
	^ self quietly: [ aClass compiledMethodAt: (aClass compile: aSource classified: 'fixtures') ]
]

{ #category : 'helpers' }
StLiveTestCase >> failureOf: aBlock [
	"The StLiveFailure that aBlock signals; the test fails when it signals none."
	^ [ aBlock value. self fail: 'no StLiveFailure was signalled' ] on: StLiveFailure do: [ :f | f ]
]

{ #category : 'helpers' }
StLiveTestCase >> loadedPackage: aName [
	"A fixture class in a package of that name: the package is loaded in the image until tearDown."
	^ self fixtureClassNamed: aName , 'Thing' superclass: Object package: aName
]
