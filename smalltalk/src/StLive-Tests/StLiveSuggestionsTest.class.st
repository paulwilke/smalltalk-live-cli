Class {
	#name : 'StLiveSuggestionsTest',
	#superclass : 'StLiveTestCase',
	#package : 'StLive-Tests'
}

{ #category : 'running' }
StLiveSuggestionsTest >> setUp [
	super setUp.
	self
		compile: 'printPaddedWith: aCharacter to: aNumber ^ aNumber' in: (self fixtureClassNamed: 'StLiveTestsSuggest' superclass: Object package: 'StLiveTestsFixture')
]

{ #category : 'helpers' }
StLiveSuggestionsTest >> suggestionsFor: aSelector [
	^ commands suggestionsFor: aSelector receiver: (Smalltalk at: #StLiveTestsSuggest) new
]

{ #category : 'suggestions' }
StLiveSuggestionsTest >> testSimilarSelectorWithASharedKeywordIsSuggested [
	| first |
	first := (self suggestionsFor: #printString:paddedWith:to:) first.
	self assert: (first at: 'selector') equals: #printPaddedWith:to:.
	self assert: (first at: 'implemented_in') equals: 'StLiveTestsSuggest'
]

{ #category : 'suggestions' }
StLiveSuggestionsTest >> testSuggestionReportsTheDistance [
	| first |
	first := (self suggestionsFor: #printString:paddedWith:to:) first.
	self assert: (first at: 'distance') equals: (commands levenshtein: 'printString:paddedWith:to:' to: 'printPaddedWith:to:')
]

{ #category : 'suggestions' }
StLiveSuggestionsTest >> testNothingSimilarSuggestsNothing [
	self assert: (self suggestionsFor: #zzqqxxyy:wwvv:) isEmpty
]

{ #category : 'suggestions' }
StLiveSuggestionsTest >> testAtMostFiveSuggestionsAreAnswered [
	| receiver |
	#(#checkA: #checkB: #checkC: #checkD: #checkE: #checkF: #checkG:) do: [ :each |
		self compile: each asString , ' aNumber ^ aNumber' in: (Smalltalk at: #StLiveTestsSuggest) ].
	receiver := (Smalltalk at: #StLiveTestsSuggest) new.
	self assert: (commands suggestionsFor: #checkZ: receiver: receiver) size equals: 5
]
