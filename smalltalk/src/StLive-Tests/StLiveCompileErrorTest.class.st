Class {
	#name : 'StLiveCompileErrorTest',
	#superclass : 'StLiveTestCase',
	#package : 'StLive-Tests'
}

{ #category : 'positions' }
StLiveCompileErrorTest >> assertPositionOf: aPosition in: aSource line: aLine column: aColumn [
	| details |
	details := commands positionDetails: aPosition in: aSource.
	self assert: (details at: 'line') equals: aLine.
	self assert: (details at: 'column') equals: aColumn
]

{ #category : 'positions' }
StLiveCompileErrorTest >> testFirstCharacterIsLineOneColumnOne [
	self assertPositionOf: 1 in: 'abc' line: 1 column: 1
]

{ #category : 'positions' }
StLiveCompileErrorTest >> testPositionOnLineThreeCountsOnlyLineFeeds [
	| source |
	source := 'one' , String lf , 'two' , String lf , 'three'.
	self assertPositionOf: 9 in: source line: 3 column: 1.
	self assertPositionOf: 11 in: source line: 3 column: 3
]

{ #category : 'positions' }
StLiveCompileErrorTest >> testMissingPositionIsTheStartOfTheSource [
	self assertPositionOf: nil in: 'abc' line: 1 column: 1
]

{ #category : 'positions' }
StLiveCompileErrorTest >> testPositionBeyondTheEndIsTheEndOfTheSource [
	self assertPositionOf: 99 in: 'ab' line: 1 column: 3
]

{ #category : 'failures' }
StLiveCompileErrorTest >> testSyntaxErrorOnLineThreeNamesLineThree [
	| cls source failure |
	cls := self fixtureClassNamed: 'StLiveTestsErrors' superclass: Object package: 'StLiveTestsFixture'.
	source := 'foo' , String lf , String tab , '| a |' , String lf , String tab , '^ a ) b'.
	failure := self failureOf: [ commands compileMethod: (commands m: { 'class' -> cls name. 'source' -> source }) ].
	self assert: failure code equals: 'syntax_error'.
	self assert: (failure details at: 'line') equals: 3
]

{ #category : 'failures' }
StLiveCompileErrorTest >> testUndeclaredVariableOnLineThreeIsNamed [
	| cls source failure |
	cls := self fixtureClassNamed: 'StLiveTestsErrors' superclass: Object package: 'StLiveTestsFixture'.
	source := 'foo' , String lf , String tab , '| a |' , String lf , String tab , '^ a + StLiveTestsNoSuchGlobal'.
	failure := self failureOf: [ commands compileMethod: (commands m: { 'class' -> cls name. 'source' -> source }) ].
	self assert: failure code equals: 'compile_error'.
	self assert: (failure details at: 'variable') equals: 'StLiveTestsNoSuchGlobal'.
	self assert: (failure details at: 'line') equals: 3.
	self assert: (failure details at: 'column') equals: 8
]

{ #category : 'failures' }
StLiveCompileErrorTest >> testUndeclaredVariableHasAHint [
	| cls failure |
	cls := self fixtureClassNamed: 'StLiveTestsErrors' superclass: Object package: 'StLiveTestsFixture'.
	failure := self failureOf: [ commands compileMethod: (commands m: { 'class' -> cls name. 'source' -> ('foo' , String lf , String tab , '^ StLiveTestsNoSuchGlobal') }) ].
	self assert: ((failure details at: 'hint') includesSubstring: 'not defined')
]

{ #category : 'failures' }
StLiveCompileErrorTest >> testFailedCompileRecordsNoChange [
	| cls before |
	cls := self fixtureClassNamed: 'StLiveTestsErrors' superclass: Object package: 'StLiveTestsFixture'.
	before := server changes size.
	self failureOf: [ commands compileMethod: (commands m: { 'class' -> cls name. 'source' -> ('foo' , String lf , String tab , '^ ( 1') }) ].
	self assert: server changes size equals: before
]

{ #category : 'failures' }
StLiveCompileErrorTest >> testCompileFailureWithoutLocationUsesTheMessage [
	| failure |
	failure := commands compileFailure: (Error new messageText: 'boom') source: 'x'.
	self assert: (failure isKindOf: StLiveFailure).
	self assert: failure code equals: 'syntax_error'.
	self assert: failure messageText equals: 'boom'.
	self assert: (failure details at: 'line') equals: 1
]

{ #category : 'failures' }
StLiveCompileErrorTest >> testCompileFailureAnswersTheFailureWithoutSignallingIt [
	| failure |
	failure := commands compileFailure: (Error new messageText: 'boom') source: 'x'.
	self assert: failure class equals: StLiveFailure
]
