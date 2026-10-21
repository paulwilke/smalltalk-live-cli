Class {
	#name : 'StLiveSourceNormalisationTest',
	#superclass : 'StLiveTestCase',
	#instVars : [
		'normalised'
	],
	#package : 'StLive-Tests'
}

{ #category : 'running' }
StLiveSourceNormalisationTest >> setUp [
	super setUp.
	normalised := self fixtureClassNamed: 'StLiveTestsNormalised' superclass: Object package: 'StLiveTestsFixture'
]

{ #category : 'helpers' }
StLiveSourceNormalisationTest >> canonicalOf: aSource [
	^ commands canonicalSourceOf: (self compile: aSource in: normalised)
]

{ #category : 'helpers' }
StLiveSourceNormalisationTest >> normalizedOf: aSource [
	^ commands normalizedSourceOf: (self compile: aSource in: normalised)
]

{ #category : 'canonical' }
StLiveSourceNormalisationTest >> testCanonicalMethodIsUnchanged [
	| source |
	source := 'foo' , String lf , String tab , '^ 1'.
	self assert: (self canonicalOf: source) equals: source
]

{ #category : 'canonical' }
StLiveSourceNormalisationTest >> testBodyOnUnaryPatternLineMovesBelowIt [
	self assert: (self canonicalOf: 'bar ^ 1') equals: 'bar' , String lf , String tab , '^ 1'
]

{ #category : 'canonical' }
StLiveSourceNormalisationTest >> testBodyOnKeywordPatternLineMovesBelowIt [
	self assert: (self canonicalOf: 'at: i put: v ^ v') equals: 'at: i put: v' , String lf , String tab , '^ v'
]

{ #category : 'canonical' }
StLiveSourceNormalisationTest >> testBodyOnManyKeywordsPatternLineMovesBelowIt [
	self assert: (self canonicalOf: 'at: i put: v with: w ^ w') equals: 'at: i put: v with: w' , String lf , String tab , '^ w'
]

{ #category : 'canonical' }
StLiveSourceNormalisationTest >> testBodyOnBinaryPatternLineMovesBelowIt [
	self assert: (self canonicalOf: '+ other ^ other') equals: '+ other' , String lf , String tab , '^ other'
]

{ #category : 'canonical' }
StLiveSourceNormalisationTest >> testBodyOnPatternLineKeepsEveryStatement [
	self assert: (self canonicalOf: 'foo ^ 1 + 2') equals: 'foo' , String lf , String tab , '^ 1 + 2'
]

{ #category : 'canonical' }
StLiveSourceNormalisationTest >> testTabsInsideTheBodyAreKept [
	| source |
	source := 'foo' , String lf , String tab , String tab , '^ 1'.
	self assert: (self canonicalOf: source) equals: source
]

{ #category : 'canonical' }
StLiveSourceNormalisationTest >> testPragmaLineIsKept [
	| source |
	source := 'foo' , String lf , String tab , '<pragmaFixture>' , String lf , String tab , '^ 1'.
	self assert: (self canonicalOf: source) equals: source
]

{ #category : 'canonical' }
StLiveSourceNormalisationTest >> testPragmaOnThePatternLineMovesBelowIt [
	self assert: (self canonicalOf: 'foo <pragmaFixture> ^ 1') equals: 'foo' , String lf , String tab , '<pragmaFixture> ^ 1'
]

{ #category : 'canonical' }
StLiveSourceNormalisationTest >> testCrLfSourceBecomesLf [
	"The CRs between the body's lines must not survive (a body moved below the pattern would keep them otherwise)."
	| source |
	source := 'foo' , String crlf , String tab , '| a |' , String crlf , String tab , '^ a'.
	self assert: (self canonicalOf: source) equals: 'foo' , String lf , String tab , '| a |' , String lf , String tab , '^ a'
]

{ #category : 'canonical' }
StLiveSourceNormalisationTest >> testMultiLineLiteralKeepsItsLineFeeds [
	| source |
	source := 'text' , String lf , String tab , '^ ''a' , String lf , 'b'''.
	self assert: (self canonicalOf: source) equals: source
]

{ #category : 'normalized' }
StLiveSourceNormalisationTest >> testNormalizedRemovesTrailingBlanksOfEachLine [
	self assert: (self normalizedOf: 'foo' , String lf , String tab , '^ 1   ' , String lf , String tab , '  ') equals: 'foo' , String lf , String tab , '^ 1'
]

{ #category : 'normalized' }
StLiveSourceNormalisationTest >> testNormalizedDropsTrailingBlankLines [
	self assert: (self normalizedOf: 'foo' , String lf , String tab , '^ 1' , String lf , String lf , String lf) equals: 'foo' , String lf , String tab , '^ 1'
]

{ #category : 'normalized' }
StLiveSourceNormalisationTest >> testNormalizedKeepsAnInnerBlankLine [
	self assert: (self normalizedOf: 'foo' , String lf , String tab , '| a |' , String lf , String lf , String tab , '^ a') equals: 'foo' , String lf , String tab , '| a |' , String lf , String lf , String tab , '^ a'
]

{ #category : 'normalized' }
StLiveSourceNormalisationTest >> testNormalizedMovesABodyOnThePatternLineBelowIt [
	self assert: (self normalizedOf: 'bar ^ 1  ') equals: 'bar' , String lf , String tab , '^ 1'
]

{ #category : 'normalized' }
StLiveSourceNormalisationTest >> testNormalizedOfCrLfSourceIsLf [
	self assert: (self normalizedOf: 'foo' , String crlf , String tab , '^ 1' , String crlf) equals: 'foo' , String lf , String tab , '^ 1'
]
