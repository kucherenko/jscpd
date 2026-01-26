class_name Enemy
extends CharacterBody2D

signal health_changed(new_health: int)
signal enemy_died

const SPEED := 100.0
const JUMP_VELOCITY := -300.0

var health: int = 50
var max_health: int = 50
var is_alive: bool = true

@onready var sprite: Sprite2D = $Sprite2D
@onready var collision: CollisionShape2D = $CollisionShape2D

func _ready() -> void:
	health = max_health
	is_alive = true
	sprite.modulate = Color.RED

func _physics_process(delta: float) -> void:
	if not is_on_floor():
		velocity += get_gravity() * delta

	if Input.is_action_just_pressed("jump") and is_on_floor():
		velocity.y = JUMP_VELOCITY

	var direction := Input.get_axis("move_left", "move_right")
	if direction:
		velocity.x = direction * SPEED
	else:
		velocity.x = move_toward(velocity.x, 0, SPEED)

	move_and_slide()

func take_damage(amount: int) -> void:
	if not is_alive:
		return
	health = max(0, health - amount)
	health_changed.emit(health)
	if health <= 0:
		die()

func die() -> void:
	is_alive = false
	enemy_died.emit()
	collision.set_deferred("disabled", true)
