Feature: Red-Black Tree
  As a kernel developer
  I want to use red-black trees
  So that I can maintain sorted data efficiently

  Scenario: Creating an empty tree
    Given an empty red-black tree
    Then the tree should be empty

  Scenario: Inserting nodes maintains balance
    Given an empty red-black tree
    When I insert a node with key 10
    And I insert a node with key 5
    And I insert a node with key 15
    Then the tree should be balanced
    And the root should be black

  Scenario: Tree maintains ordering
    Given an empty red-black tree
    When I insert multiple nodes
    Then in-order traversal should be sorted
