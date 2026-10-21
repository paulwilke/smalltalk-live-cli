Class {
	#name : 'StLiveCommandsLineEndingsTest',
	#superclass : 'StLiveTestCase',
	#package : 'StLive-Tests'
}

{ #category : 'lf' }
StLiveCommandsLineEndingsTest >> testLfOfNilIsNil [
	self assert: (commands lf: nil) isNil
]

{ #category : 'lf' }
StLiveCommandsLineEndingsTest >> testLfLeavesLfUnchanged [
	self assert: (commands lf: 'a' , String lf , 'b') equals: 'a' , String lf , 'b'
]

{ #category : 'lf' }
StLiveCommandsLineEndingsTest >> testLfConvertsCrLf [
	self assert: (commands lf: 'a' , String crlf , 'b') equals: 'a' , String lf , 'b'
]

{ #category : 'lf' }
StLiveCommandsLineEndingsTest >> testLfConvertsLoneCr [
	self assert: (commands lf: 'a' , String cr , 'b') equals: 'a' , String lf , 'b'
]

{ #category : 'lf' }
StLiveCommandsLineEndingsTest >> testLfNormalisesMixedEndings [
	self
		assert: (commands lf: 'a' , String crlf , 'b' , String cr , 'c' , String lf , 'd')
		equals: 'a' , String lf , 'b' , String lf , 'c' , String lf , 'd'
]

{ #category : 'lf' }
StLiveCommandsLineEndingsTest >> testLfKeepsATrailingLineEnding [
	self assert: (commands lf: 'x' , String crlf) equals: 'x' , String lf
]

{ #category : 'lf' }
StLiveCommandsLineEndingsTest >> testLfOfEmptyStringIsEmpty [
	self assert: (commands lf: '') isEmpty
]

{ #category : 'lf' }
StLiveCommandsLineEndingsTest >> testLfKeepsTabsAndOtherCharacters [
	self assert: (commands lf: (String with: Character tab with: $a with: Character cr)) equals: (String with: Character tab with: $a with: Character lf)
]

{ #category : 'compileMethod' }
StLiveCommandsLineEndingsTest >> testCompiledMultiLineLiteralHoldsCr [
	| cls source |
	cls := self fixtureClassNamed: 'StLiveTestsLiteral' superclass: Object package: 'StLiveTestsFixture'.
	source := 'text' , String lf , String tab , '^ ''first' , String lf , 'second'''.
	commands compileMethod: (commands m: { 'class' -> cls name. 'source' -> source. 'protocol' -> 'test' }).
	self assert: (cls new text includes: Character cr).
	self deny: (cls new text includes: Character lf)
]

{ #category : 'compileMethod' }
StLiveCommandsLineEndingsTest >> testCompileMethodRecordsTheLfSource [
	| cls source |
	cls := self fixtureClassNamed: 'StLiveTestsLiteral' superclass: Object package: 'StLiveTestsFixture'.
	source := 'text' , String lf , String tab , '^ ''first' , String lf , 'second'''.
	commands compileMethod: (commands m: { 'class' -> cls name. 'source' -> source. 'protocol' -> 'test' }).
	self assert: (server changes last at: 'new_source') equals: source.
	self deny: ((server changes last at: 'new_source') includes: Character cr)
]

{ #category : 'compileMethod' }
StLiveCommandsLineEndingsTest >> testCompileMethodAnswersTheMethodDescription [
	| cls response result |
	cls := self fixtureClassNamed: 'StLiveTestsCompiled' superclass: Object package: 'StLiveTestsFixture'.
	response := commands compileMethod: (commands m: { 'class' -> cls name. 'source' -> ('answer' , String lf , String tab , '^ 42'). 'protocol' -> 'test' }).
	result := response at: 'result'.
	self assert: (response at: 'ok').
	self assert: (result at: 'selector') equals: #answer.
	self assert: (result at: 'created').
	self assert: (result at: 'change_index') equals: 1
]

{ #category : 'compileMethod' }
StLiveCommandsLineEndingsTest >> testCompileMethodRecordsAMethodCompileChange [
	| cls entry |
	cls := self fixtureClassNamed: 'StLiveTestsCompiled' superclass: Object package: 'StLiveTestsFixture'.
	commands compileMethod: (commands m: { 'class' -> cls name. 'source' -> ('answer' , String lf , String tab , '^ 42'). 'protocol' -> 'test' }).
	entry := server changes last.
	self assert: (entry at: 'kind') equals: 'method-compile'.
	self assert: (entry at: 'selector') equals: #answer.
	self assert: (entry at: 'protocol') equals: 'test'.
	self assert: (entry at: 'saved') not
]

{ #category : 'compileMethod' }
StLiveCommandsLineEndingsTest >> testRecompilingTheSameSourceIsUnchanged [
	| cls args second |
	cls := self fixtureClassNamed: 'StLiveTestsCompiled' superclass: Object package: 'StLiveTestsFixture'.
	args := commands m: { 'class' -> cls name. 'source' -> ('answer' , String lf , String tab , '^ 42'). 'protocol' -> 'test' }.
	commands compileMethod: args.
	second := (commands compileMethod: args) at: 'result'.
	self assert: (second at: 'unchanged').
	self deny: (second at: 'created')
]
