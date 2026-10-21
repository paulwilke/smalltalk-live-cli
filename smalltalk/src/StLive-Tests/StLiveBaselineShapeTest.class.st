Class {
	#name : 'StLiveBaselineShapeTest',
	#superclass : 'StLiveTestCase',
	#package : 'StLive-Tests'
}

{ #category : 'groups' }
StLiveBaselineShapeTest >> testDefaultGroupLoadsOnlyTheFramework [
	| version |
	version := (Smalltalk at: #BaselineOfStLive) version.
	self assert: ((version packagesForSpecNamed: 'default') collect: [ :p | p name asString ]) asArray equals: #('StLive')
]

{ #category : 'groups' }
StLiveBaselineShapeTest >> testTestsGroupAddsTheTestPackage [
	| names |
	names := ((Smalltalk at: #BaselineOfStLive) version packagesForSpecNamed: 'Tests') collect: [ :p | p name asString ].
	self assert: names asSet equals: #('StLive' 'StLive-Tests') asSet
]

{ #category : 'groups' }
StLiveBaselineShapeTest >> testTestPackageRequiresTheFramework [
	| spec |
	spec := (Smalltalk at: #BaselineOfStLive) version packageNamed: 'StLive-Tests'.
	self assert: (spec requires collect: [ :name | name asString ]) asArray equals: #('StLive')
]
