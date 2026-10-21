Class {
	#name : 'StLiveCommentRecordingTest',
	#superclass : 'StLiveTestCase',
	#instVars : [
		'commented'
	],
	#package : 'StLive-Tests'
}

{ #category : 'running' }
StLiveCommentRecordingTest >> setUp [
	super setUp.
	commented := self fixtureClassNamed: 'LiveTestsCommented' superclass: Object package: 'StLiveTestsFixture'
]

{ #category : 'helpers' }
StLiveCommentRecordingTest >> commentAnnouncementFor: aClass [
	^ ClassCommented new classCommented: aClass; yourself
]

{ #category : 'comment' }
StLiveCommentRecordingTest >> testCommentOfAClassWithAComment [
	self quietly: [ commented comment: 'A bar.' ].
	self assert: (commands commentOf: commented) equals: 'A bar.'
]

{ #category : 'comment' }
StLiveCommentRecordingTest >> testCommentOfAClassWithoutCommentIsEmpty [
	self assert: (commands commentOf: commented) equals: ''
]

{ #category : 'comment' }
StLiveCommentRecordingTest >> testCommentOfAnEmptyCommentIsEmpty [
	self quietly: [ commented comment: '' ].
	self assert: (commands commentOf: commented) equals: ''
]

{ #category : 'comment' }
StLiveCommentRecordingTest >> testCommentKeepsQuotesAsTheyAre [
	self quietly: [ commented comment: 'Say "hi" and ''bye''.' ].
	self assert: (commands commentOf: commented) equals: 'Say "hi" and ''bye''.'
]

{ #category : 'comment' }
StLiveCommentRecordingTest >> testCommentOfNilIsNil [
	self assert: (commands commentOf: nil) isNil
]

{ #category : 'recording' }
StLiveCommentRecordingTest >> testClassCommentedIsRecordedAsAClassChange [
	| entry |
	self quietly: [ commented comment: 'A bar.' ].
	server classChanged: (self commentAnnouncementFor: commented).
	self assert: server changes size equals: 1.
	entry := server changes last.
	self assert: (entry at: 'kind') equals: 'class-create'.
	self assert: (entry at: 'class_name') equals: commented name.
	self assert: (entry at: 'comment') equals: 'A bar.'.
	self assert: (entry at: 'origin') equals: 'eval'.
	self assert: (entry at: 'saved') not
]

{ #category : 'recording' }
StLiveCommentRecordingTest >> testClassCommentedWithoutCommentRecordsEmptyComment [
	server classChanged: (self commentAnnouncementFor: commented).
	self assert: (server changes last at: 'comment') equals: ''
]

{ #category : 'recording' }
StLiveCommentRecordingTest >> testCommentOfAMetaclassAnnouncementIsIgnored [
	server classChanged: (self commentAnnouncementFor: commented class).
	self assert: server changes isEmpty
]

{ #category : 'recording' }
StLiveCommentRecordingTest >> testCommentOfAnStLiveClassIsIgnored [
	server classChanged: (self commentAnnouncementFor: StLiveServer).
	self assert: server changes isEmpty
]

{ #category : 'recording' }
StLiveCommentRecordingTest >> testSuppressedCommentIsNotRecorded [
	server suppressRecordingDuring: [ server classChanged: (self commentAnnouncementFor: commented) ].
	self assert: server changes isEmpty
]

{ #category : 'recording' }
StLiveCommentRecordingTest >> testRealCommentChangeIsRecordedBySubscribedServer [
	server subscribeToSystemChanges.
	self quietly: [ commented comment: 'Changed.' ].
	self assert: server changes size equals: 1.
	self assert: (server changes last at: 'comment') equals: 'Changed.'
]
