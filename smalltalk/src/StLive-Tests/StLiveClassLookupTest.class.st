Class {
	#name : 'StLiveClassLookupTest',
	#superclass : 'StLiveTestCase',
	#package : 'StLive-Tests'
}

{ #category : 'classNamed' }
StLiveClassLookupTest >> testClassNamedFindsAGlobalClass [
	self assert: (commands classNamed: 'Object') equals: Object
]

{ #category : 'classNamed' }
StLiveClassLookupTest >> testClassNamedTrimsBlanks [
	self assert: (commands classNamed: '  Object  ') equals: Object
]

{ #category : 'classNamed' }
StLiveClassLookupTest >> testClassNamedClassSuffixMeansTheMetaclass [
	self assert: (commands classNamed: 'Object class') equals: Object class.
	self assert: (commands classNamed: 'Object classSide') equals: Object class
]

{ #category : 'classNamed' }
StLiveClassLookupTest >> testClassNamedTraitSideMeansItsMetaclass [
	| trait |
	trait := self fixtureTraitNamed: 'TStLiveTestsLookup' package: 'StLiveTestsFixture'.
	self assert: (commands classNamed: 'TStLiveTestsLookup classSide') equals: trait class
]

{ #category : 'classNamed' }
StLiveClassLookupTest >> testClassNamedUnknownNameFails [
	| failure |
	failure := self failureOf: [ commands classNamed: 'StLiveTestsNoSuchClass' ].
	self assert: failure code equals: 'unknown_class'.
	self assert: failure messageText equals: 'No class named StLiveTestsNoSuchClass'
]

{ #category : 'classNamed' }
StLiveClassLookupTest >> testClassNamedOfANonClassGlobalFails [
	| failure |
	failure := self failureOf: [ commands classNamed: 'Smalltalk' ].
	self assert: failure code equals: 'unknown_class'.
	self assert: (failure messageText includesSubstring: 'not a class')
]

{ #category : 'kind' }
StLiveClassLookupTest >> testKindOfClassAndTrait [
	| trait |
	trait := self fixtureTraitNamed: 'TStLiveTestsKind' package: 'StLiveTestsFixture'.
	self assert: (commands kindOf: Object) equals: 'class'.
	self assert: (commands kindOf: Object class) equals: 'class'.
	self assert: (commands kindOf: trait) equals: 'trait'
]

{ #category : 'traits' }
StLiveClassLookupTest >> testTraitCompositionOfAPlainClassIsNil [
	self assert: (commands traitCompositionOf: Object) isNil
]

{ #category : 'traits' }
StLiveClassLookupTest >> testTraitCompositionNamesTheTraitsOfAClass [
	| trait cls |
	trait := self fixtureTraitNamed: 'TStLiveTestsComposed' package: 'StLiveTestsFixture'.
	cls := self quietly: [
		self class classInstaller make: [ :builder |
			builder superclass: Object; name: #StLiveTestsUsesTrait; traits: trait; package: 'StLiveTestsFixture' ] ].
	fixtureClasses add: cls.
	self assert: (commands traitCompositionOf: cls) equals: 'TStLiveTestsComposed'
]
